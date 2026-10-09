mod moisture_sensor;

use embassy_time::{Duration, Timer};
use esp_idf_svc::hal::{
    adc::{oneshot::AdcDriver, AdcChannel},
    gpio::{Output, PinDriver},
    peripherals::Peripherals,
    task::block_on,
};
use tm1637_embedded_hal::{formatters::i16_to_4digits, tokens::Async, Brightness, TM1637Builder, TM1637};
use crate::moisture_sensor::MoistureSensor;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

type TM1637DisplayT = TM1637<4, Async, PinDriver<'static, Output>, PinDriver<'static, Output>, embassy_time::Delay>;

async fn sensor_display_loop<C: AdcChannel>(
    mut moisture_sensor: MoistureSensor<'_, C>,
    mut display: TM1637DisplayT,
) {
    if let Err(e) = display.init().await {
        log::error!("failed to init display: {e:?}");
        return;
    }

    loop {
        match moisture_sensor.read() {
            Ok(percent) => {
                log::info!("{percent}");
                let segments = i16_to_4digits(percent as i16);
                if let Err(e) = display.display_slice(0, &segments).await {
                    log::error!("failed to write to display: {e:?}");
                }
            }
            Err(e) => log::error!("failed to read sensor: {e:?}"),
        }

        Timer::after(POLL_INTERVAL).await;
    }
}

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
    let moisture_sensor = MoistureSensor::new(peripherals.pins.gpio0, &adc1)?;

    // Create a TM1637 instance
    let tm1637_clk = PinDriver::output(peripherals.pins.gpio5)?;
    let tm1637_dio = PinDriver::output(peripherals.pins.gpio6)?;
    // ::<4> is for 4-digit displays
    let display = TM1637Builder::new(tm1637_clk, tm1637_dio, embassy_time::Delay)
        .brightness(Brightness::L5)
        .build_async::<4>();

    block_on(sensor_display_loop(moisture_sensor, display));

    Ok(())
}
