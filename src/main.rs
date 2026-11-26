mod config;
mod display;
mod logging;
mod moisture_sensor;
mod restart;
mod sntp;
mod wifi;

use crate::{
    display::TM1637Screen,
    moisture_sensor::{MoistureGauge, MoistureSensor},
};
use anyhow::Context as _;
use embassy_futures::select::select_array;
use embassy_time::Duration;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{adc::oneshot::AdcDriver, gpio::PinDriver, peripherals::Peripherals, task::block_on},
    wifi::WifiDeviceId,
};
use fak_core::{moisture::Calibration, topics::DeviceId};
use std::{
    convert::Infallible,
    pin::Pin,
    time::{SystemTime, UNIX_EPOCH},
};
use tm1637_embedded_hal::{Brightness, TM1637Builder};

// TODO: This calibration should instead be capable of being modified on runtime via MQTT, and not a const
const MOISTURE_SENSOR_01_CALIBRATION: Calibration = match Calibration::new(2309, 780) {
    Ok(calibration) => calibration,
    Err(_) => panic!("Moisture sensor 01: dry reading must be above wet reading"),
};

const TASK_NAMES: [&str; 3] = ["moisture_sensor", "display", "WiFi monitor"];

/// How long boot waits for the clock to sync before `run` fails and the device restarts.
/// The clock synced in about 6 s on the bench; 30 s leaves room for a slow network.
const SNTP_SYNC_TIMEOUT: Duration = Duration::from_secs(30);

fn run() -> anyhow::Result<Infallible> {
    logging::init().context("initializing logging")?;

    log::info!("Welcome to lector humedad!");
    log::info!("MQTT host: {}", config::MQTT_HOST);
    log::info!("WiFi SSID: {}", config::WIFI_SSID);

    // Peripherals::take() , using a singleton, makes sure you dont try to interact with a pin in two different pieces of code
    let peripherals = Peripherals::take().context("taking hold of peripherals")?;

    // Wifi
    let sysloop = EspSystemEventLoop::take().context("taking the system event loop")?;
    // NEVER DROP `wifi_driver` !!!
    // It holds the wifi connection, if you drop it, you break the internet connection
    let mut wifi_driver = wifi::connect(
        config::WIFI_SSID,
        config::WIFI_PASSWORD,
        peripherals.modem,
        sysloop,
    )
    .context("connecting to WiFi")?;

    let mac = wifi_driver
        .get_mac(WifiDeviceId::Sta)
        .context("reading the WiFi station MAC address")?;
    let device_id = DeviceId::from_mac(mac);
    log::info!("device id: {device_id}");

    let _sntp =
        sntp::sync(config::SNTP_SERVER, SNTP_SYNC_TIMEOUT).context("syncing time over SNTP")?;

    let unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("reading the system clock")?;
    log::info!(
        "clock synced over SNTP: {} s since 1970 (UTC)",
        unix_time.as_secs()
    );

    // AdcDriver owns the ADC.
    // there's just one usable unit on the ESP32 C3: adc1. It is used regardless of which GPIO between 0 and 4 is used
    // adc2 does exist, but its usage its not recommended,
    // see https://docs.espressif.com/projects/esp-chip-errata/en/latest/esp32c3/03-errata-description/index.html
    let adc1 = AdcDriver::new(peripherals.adc1).context("creating the ADC1 driver")?;

    // When choosing pins, remember:
    // - GPIO 0-4   --> ADC pins
    // - GPIO 9     --> Strapping pin (BOOT) (TRY TO AVOID, see links)
    // - GPIO 2,8   --> Strapping pin (BOOT) (TRY TO AVOID, see links)
    // - GPIO 20/21 --> UART RX/TX pins
    // See https://www.espboards.dev/blog/esp32-strapping-pins/
    // See https://esp32.com/viewtopic.php?p=83835

    let moisture_gauge = MoistureGauge::new("moisture-01");
    let mut moisture_sensor = MoistureSensor::new(
        MOISTURE_SENSOR_01_CALIBRATION,
        peripherals.pins.gpio0,
        &adc1,
        Duration::from_millis(100),
    )
    .context("creating the moisture sensor")?;

    let tm1637_clk = PinDriver::output(peripherals.pins.gpio5)
        .context("setting GPIO5 as output for TM1637 CLK")?;
    let tm1637_dio = PinDriver::output(peripherals.pins.gpio6)
        .context("setting GPIO6 as output for TM1637 DIO")?;
    // ::<4> is for 4-digit displays
    let tm1637 = TM1637Builder::new(tm1637_clk, tm1637_dio, embassy_time::Delay)
        .brightness(Brightness::L5)
        .build_async::<4>();

    let mut screen = block_on(TM1637Screen::init(tm1637))
        .map_err(|e| anyhow::anyhow!("{e:?}"))
        .context("creating the TM1637 screen")?;

    // The tasks never end. If one does, it's a bug.
    // If you add more tasks, please modify TASK_NAMES too
    let tasks: [Pin<Box<dyn Future<Output = ()> + '_>>; 3] = [
        Box::pin(moisture_sensor.run(&moisture_gauge)),
        Box::pin(display::display_loop(&moisture_gauge, &mut screen)),
        Box::pin(wifi::monitor(&mut wifi_driver)),
    ];
    let ((), index) = block_on(select_array(tasks));

    // Normally, the code just loops, and nothing reaches beyond here.
    // --- //
    Err(anyhow::anyhow!(
        "the {} task ended, but tasks run forever",
        TASK_NAMES[index]
    ))
}

fn main() {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    match run() {
        Err(e) => restart::fail_and_restart(&e),
        // Because run() its Infallible, match does not ask for Ok(). Pretty neat huh
    }
}
