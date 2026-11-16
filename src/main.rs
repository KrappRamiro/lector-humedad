mod display;
mod moisture_sensor;

use std::pin::Pin;

use crate::{
    display::TM1637Screen,
    moisture_sensor::{MoistureGauge, MoistureSensor},
};
use embassy_futures::select::select_array;
use embassy_time::Duration;
use esp_idf_svc::hal::{
    adc::oneshot::AdcDriver, gpio::PinDriver, peripherals::Peripherals, task::block_on,
};
use fak_core::moisture::Calibration;
use tm1637_embedded_hal::{Brightness, TM1637Builder};

// TODO: This calibration should instead be capable of being modified on runtime via MQTT, and not a const
const MOISTURE_SENSOR_01_CALIBRATION: Calibration = match Calibration::new(2309, 780) {
    Ok(calibration) => calibration,
    Err(_) => panic!("Moisture sensor 01: dry reading must be above wet reading"),
};
const TASK_NAMES: [&str; 2] = ["moisture_sensor", "display"];

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Init a logger that uses Debug as the target level
    let logger = esp_idf_svc::log::init_from_esp_idf();
    // lector_humedad is the Rust project name, but with - converted to _
    logger
        .filter()
        .set_target_level("lector_humedad", log::LevelFilter::Debug)?;
    logger
        .filter()
        .set_target_level("lector_humedad::moisture_sensor", log::LevelFilter::Debug)?;

    log::info!("Hello, world!");

    // Peripherals::take() , using a singleton, makes sure you dont try to interact with a pin in two different pieces of code
    let peripherals = Peripherals::take()?;

    // AdcDriver owns the ADC.
    // there's just one usable unit on the ESP32 C3: adc1. It is used regardless of which GPIO between 0 and 4 is used
    // adc2 does exist, but its usage its not recommended,
    // see https://docs.espressif.com/projects/esp-chip-errata/en/latest/esp32c3/03-errata-description/index.html
    let adc1 = AdcDriver::new(peripherals.adc1)?;

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
    )?;

    let tm1637_clk = PinDriver::output(peripherals.pins.gpio5)?;
    let tm1637_dio = PinDriver::output(peripherals.pins.gpio6)?;
    // ::<4> is for 4-digit displays
    let tm1637 = TM1637Builder::new(tm1637_clk, tm1637_dio, embassy_time::Delay)
        .brightness(Brightness::L5)
        .build_async::<4>();

    let mut screen = block_on(TM1637Screen::init(tm1637)).map_err(|e| anyhow::anyhow!("{e:?}"))?;

    // The tasks never end. If one does, it's a bug.
    // If you add more tasks, please modify TASK_NAMES too
    let tasks: [Pin<Box<dyn Future<Output = ()> + '_>>; 2] = [
        Box::pin(moisture_sensor.run(&moisture_gauge)),
        Box::pin(display::display_loop(&moisture_gauge, &mut screen)),
    ];
    let ((), index) = block_on(select_array(tasks));

    // Normally, the code just loops, and nothing reaches beyond here.
    // --- //
    Err(anyhow::anyhow!(
        "the {} task ended, but tasks run forever",
        TASK_NAMES[index]
    ))
}
