use anyhow::Context;
use embassy_time::{Duration, Instant, Timer};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{delay::FreeRtos, modem::Modem},
    nvs::EspDefaultNvsPartition,
    sys::{EspError, esp, esp_wifi_set_max_tx_power},
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use fak_core::link::{Action, DOWN_LIMIT, LinkMonitor};

use crate::restart;

/// Maximum transmit power, in units of 0.25 dBm. The driver rounds it down to a
/// supported step: 30 falls in `[28, 33]`, which gives 28, so 7 dBm
/// (`esp_wifi.h` in ESP-IDF v6.1).
/// Some ESP32-C3 Super Mini boards have a bad antenna design and can't connect at full power.
/// See <https://forum.arduino.cc/t/no-wifi-connect-with-esp32-c3-super-mini/1324046/24>
const MAX_TX_POWER: i8 = 30;

/// How many times `connect` tries to join the network before giving up.
const MAX_CONNECT_ATTEMPTS: u8 = 5;

/// Pause between two connection attempts, in milliseconds.
const RETRY_DELAY_MS: u32 = 500;

/// How often `monitor` checks the wifi link
const CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Connects to the given WiFi network and waits until it has an IP address.
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
            .context("WiFi password is longer than 64 bytes")?,
        auth_method: AuthMethod::WPA2Personal, // el modo de auth mas debil aceptado.
        ..Default::default()
    });
    wifi.set_configuration(&config)
        .context("setting the WiFi configuration")?;
    wifi.start().context("starting the WiFi driver")?;

    // al ESP32 C3 hay que bajarle la potencia, la antena que tiene por default es una verga
    // SAFETY: This is safe, because in the previous lines, the driver was initialized (EspWifi::new) and started (wifi.start), which the function requires.
    esp!(unsafe { esp_wifi_set_max_tx_power(MAX_TX_POWER) })
        .context("lowering the WiFi transmit power")?;

    // Logic for connecting to internet with retry incorporated
    for attempt in 1..=MAX_CONNECT_ATTEMPTS {
        log::info!("connecting to WiFi, attempt {attempt}/{MAX_CONNECT_ATTEMPTS}");
        let result = wifi
            .connect()
            .context("connecting to the WiFi network")
            // If everything ok, wait for DHCP and the underlying network interface
            .and_then(|()| wifi.wait_netif_up().context("waiting for an IP address"));

        match result {
            Ok(()) => break,
            Err(e) if attempt < MAX_CONNECT_ATTEMPTS => {
                log::warn!("WiFi attempt {attempt} failed: {e:#}");
                // Leaves the driver in a clean "not connected" state for the next attempt.
                // This could help if, for example, wait_netif_up failed leaving us associated with the router, but without any IP
                // A failed disconnect must not end the retry loop, so log it and keep going.
                if let Err(e) = wifi.disconnect() {
                    log::warn!("disconnecting before retrying failed: {e}");
                }
                FreeRtos::delay_ms(RETRY_DELAY_MS);
            }
            Err(e) => return Err(e),
        }
    }

    let ip_info = wifi
        .wifi()
        .sta_netif()
        .get_ip_info()
        .context("reading the station IP address")?;
    log::info!("WiFi up, IP {}", ip_info.ip);

    // Al final del día, es EspWifi el dueño de la conexión. Mientras este valor viva (y no sea droppeado), la conexión va a seguir viva.
    Ok(esp_wifi)
}

/// Watches the wifi link forever.
/// Reconnects after a drop, and restarts the board if the link stays down for `DOWN_LIMIT`
pub async fn monitor(wifi: &mut EspWifi<'static>) {
    let mut link = LinkMonitor::default();

    loop {
        Timer::after(CHECK_INTERVAL).await;

        // An error reading the state counts as down.
        let is_up = matches!(wifi.is_up(), Ok(true));
        // we need core::time::duration, and not embassy_time::Duration,
        // because fak_core uses core:: and not embassy_time::
        let now = core::time::Duration::from_millis(Instant::now().as_millis());

        match link.step(is_up, now) {
            Action::Wait => {}
            Action::Reconnect => {
                log::warn!("WiFi link lost, reconnecting");
                if let Err(e) = reconnect(wifi) {
                    log::warn!("Reconnecting to wifi failed: {e}");
                }
            }
            Action::Retry => {
                if let Err(e) = reconnect(wifi) {
                    log::debug!("Reconnecting to wifi failed again: {e}");
                }
            }
            Action::Recovered => log::info!("Wifi link back"),
            Action::Restart => restart::fail_and_restart(&anyhow::anyhow!(
                "WiFi was down for {} s",
                DOWN_LIMIT.as_secs()
            )),
        }
    }
}

fn reconnect(wifi: &mut EspWifi<'static>) -> Result<(), EspError> {
    if matches!(wifi.is_connected(), Ok(true)) {
        return Ok(());
    }
    wifi.connect()
}
