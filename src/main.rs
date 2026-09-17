use std::{thread, time::Duration};

use esp_idf_svc::hal::{
    adc::attenuation,
    adc::oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    peripherals::Peripherals,
};

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("Hello, world!");

    // --- Reading Analog input --- //
    // Please read https://github.com/esp-rs/esp-idf-hal/blob/master/examples/adc_oneshot.rs

    // Peripherals::take() , using a singleton, makes sure you dont try to interact with a pin in two different pieces of code
    let peripherials = Peripherals::take()?;

    // AdcDriver owns the ADC.
    // there's just one usable unit on the ESP32 C3: adc1. It is used regardless of which GPIO between 0 and 4 is used
    // adc2 does exist, but its usage its not recommended,
    // see https://docs.espressif.com/projects/esp-chip-errata/en/latest/esp32c3/03-errata-description/index.html
    let adc1 = AdcDriver::new(peripherials.adc1)?;

    // configuring pin to analog read, you can regulate the adc input voltage range depending on your need
    // for this example we use the attenuation of 11db which sets the input voltage range to around 0-3.6V
    let adc1_config = AdcChannelConfig {
        attenuation: attenuation::DB_12,
        ..Default::default()
    };

    let mut adc_pin = AdcChannelDriver::new(&adc1, peripherials.pins.gpio0, &adc1_config)?;

    loop {
        // you can change the sleep duration depending on how often you want to sample
        thread::sleep(Duration::from_millis(100));
        println!("ADC value: {}", adc1.read(&mut adc_pin)?);
    }
}
