use anyhow::Context;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{delay::FreeRtos, modem::Modem},
    nvs::EspDefaultNvsPartition,
    sys::{esp, esp_wifi_set_max_tx_power},
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};

/// Maximum transmit power, in units of 0.25 dBm. 34 is 8.5 dBm.
/// Some ESP32-C3 Super Mini boards have a bad antenna design and can't connect at full power.
/// See https://forum.arduino.cc/t/no-wifi-connect-with-esp32-c3-super-mini/1324046/24
const MAX_TX_POWER: i8 = 34;

/// How many times `connect` tries to join the network before giving up.
const MAX_CONNECT_ATTEMPTS: u8 = 5;

/// Pause between two connection attemps, in milliseconds.
const RETRY_DELAY_MS: u32 = 500;

/// Connects to the wifi specified in cfg.toml
pub fn connect(
    ssid: &str,
    password: &str,
    modem: Modem<'static>, // singleton del periférico de radio (WiFi + Bluetooth). Lo consume EspWifi
    sysloop: EspSystemEventLoop, // es un handle al event loop de ESP IDF. A diferencia de Modem, se puede clonar para obtener otro handle
) -> anyhow::Result<EspWifi<'static>> {
    // NVS RF calibration
    // See https://docs.espressif.com/projects/esp-idf/en/v6.1/esp32c3/api-guides/RF_calibration.html
    let nvs = EspDefaultNvsPartition::take().context("taking the default NVS partition")?;
    // driver wifi
    // EspWifi es una capa de Rust que se encarga de llamar a las funciones de C que interactuan con el hardware de Wifi.
    // Estas funciones de C publican sus eventos en el event loop (EspSystemEventLoop).
    let mut esp_wifi =
        EspWifi::new(modem, sysloop.clone(), Some(nvs)).context("creating the WiFi driver")?;
    // wrapper wifi
    // BlockingWifi lo que hace es leer el event loop (EspSystemEventLoop) y bloquear en cada operación (start, connect, disconnect)
    let mut wifi =
        BlockingWifi::wrap(&mut esp_wifi, sysloop).context("wrapping the WiFi driver")?;

    // Config for wifi in STA mode (client mode)
    let config = Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().context("SSID is longer than 32 bytes")?,
        password: password
            .try_into()
            .context("Wifi password is longer than 64 bytes")?,
        auth_method: AuthMethod::WPA2Personal, // el modo de auth mas debil aceptado.
        ..Default::default()
    });
    wifi.set_configuration(&config)
        .context("setting the WiFi configuration")?;
    wifi.start().context("Starting the wifi")?;

    // al ESP32 C3 hay que bajarle la potencia, la antena que tiene por default es una verga
    // SAFETY: This is safe, because in the previous lines, the driver was initialized (EspWifi::new) and started (wifi.start), which the function requires.
    esp!(unsafe { esp_wifi_set_max_tx_power(MAX_TX_POWER) })
        .context("Lowering the Wifi transmit power")?;

    // Logic for connecting to internet with retry incorporated
    for attempt in 1..=MAX_CONNECT_ATTEMPTS {
        log::info!("connnecting to wifi, attempt {attempt}/{MAX_CONNECT_ATTEMPTS}");
        let result = wifi
            .connect()
            .context("connecting to the WiFi network")
            // If everything ok, wait for DHCP and the underlying network interface
            .and_then(|()| wifi.wait_netif_up().context("Waiting for an IP address"));

        match result {
            Ok(()) => break,
            Err(e) if attempt < MAX_CONNECT_ATTEMPTS => {
                log::warn!("Wifi attempt {attempt} failed: {e:#}");
                // Leaves the driver in a clean "not connected" state afor the next attempt.
                // This could help if, for example, wait_netif_up failed leaving us associated with the router, but without any IP
                wifi.disconnect().context("disconnecting before retrying")?;
                FreeRtos::delay_ms(RETRY_DELAY_MS);
            }
            Err(e) => return Err(e),
        }
    }

    // Al final del día, es EspWifi el dueño de la conexión. Mientras este valor viva (y no sea droppeado), la conexión va a seguir viva.
    Ok(esp_wifi)
}
