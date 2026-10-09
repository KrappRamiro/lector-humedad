mod moisture_sensor;
use std::{thread, time::Duration};

use esp_idf_svc::hal::{
    adc::oneshot::AdcDriver, delay::Delay, gpio::PinDriver, peripherals::Peripherals,
};
use tm1637_embedded_hal::TM1637Builder;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
use crate::moisture_sensor::MoistureSensor;

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

    // --- Reading Analog input --- //
    // Please read https://github.com/esp-rs/esp-idf-hal/blob/master/examples/adc_oneshot.rs

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

    let mut moisture_sensor = MoistureSensor::new(peripherals.pins.gpio0, &adc1)?;


    loop {
        let reading = moisture_sensor.read()?;
        log::info!("{}", reading);

        thread::sleep(POLL_INTERVAL);
    }
}
