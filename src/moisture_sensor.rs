use std::{
    cell::Cell,
    time::{SystemTime, UNIX_EPOCH},
};

use embassy_time::{Duration, Timer};
use esp_idf_svc::hal::{
    adc::{
        AdcChannel, attenuation,
        oneshot::{AdcChannelDriver, AdcDriver, config::AdcChannelConfig},
    },
    gpio::ADCPin,
};
use fak_core::{
    gauge::{Gauge, Reading, Value},
    moisture::Calibration,
};

#[derive(Clone, Copy, Debug)]
struct Sample {
    millivolts: u16,
    percent: u8,
    ts: u64,
}

pub struct MoistureGauge {
    name: &'static str,
    latest: Cell<Option<Sample>>,
}

impl MoistureGauge {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            latest: Cell::new(None),
        }
    }

    /// Stores the latest sample, or `None` after a failed read.
    /// Used by the sensors to tell the gauges their readings.
    fn store(&self, sample: Option<Sample>) {
        self.latest.set(sample);
    }
}

impl Gauge for MoistureGauge {
    fn name(&self) -> &'static str {
        self.name
    }

    fn latest(&self) -> Option<Reading> {
        self.latest.get().map(|s| Reading {
            value: Value::from(s.percent),
            ts: s.ts,
        })
    }
}

/// Reads the soil moisture sensor through one ADC channel.
///
/// - `Pin`: the physical pin on the board.
/// - `AdcUnit`: the ADC hardware block in the chip (ADC1).
/// - `AdcChannel`: a pin plus the ADC unit it is wired to. Generic `C` below.
/// - `AdcDriver`: a claim over a whole ADC unit, independent of any pin.
/// - `AdcChannelDriver`: the same, but for one channel.
///
/// See `docs/notes/adc-types.md` for how these types fit together.
pub struct MoistureSensor<'d, C: AdcChannel> {
    calibration: Calibration,
    /// Borrows the `AdcDriver` instead of owning it: one ADC unit serves several
    /// pins, so an owner would lock the others out.
    /// `AdcChannelDriver`'s third parameter accepts anything that borrows as
    /// `AdcDriver`; we pass `&AdcDriver`.
    adc_channel_driver: AdcChannelDriver<'d, C, &'d AdcDriver<'d, C::AdcUnit>>,
    /// Time between samples
    refresh_interval: Duration,
}

impl<'d, C: AdcChannel> MoistureSensor<'d, C> {
    pub fn new(
        calibration: Calibration,
        // Any pin wired to the ADC channel `C`.
        pin: impl ADCPin<AdcChannel = C> + 'd,
        // The ADC unit that channel belongs to.
        adc: &'d AdcDriver<'d, C::AdcUnit>,
        refresh_interval: Duration,
    ) -> anyhow::Result<Self> {
        // `DB_12` is the widest input range; the sensor outputs up to 3.0 V.
        let adc_config = AdcChannelConfig {
            attenuation: attenuation::DB_12,
            ..Default::default()
        };

        let adc_channel_driver = AdcChannelDriver::new(adc, pin, &adc_config)?;

        Ok(Self {
            calibration,
            adc_channel_driver,
            refresh_interval,
        })
    }
    fn sample(&mut self) -> anyhow::Result<Sample> {
        const READING_ITERATIONS: u32 = 30;
        let mut total_sum: u32 = 0;
        for _ in 0..READING_ITERATIONS {
            let reading = self.adc_channel_driver.read()?;
            total_sum += u32::from(reading);
        }
        let reading_avg = u16::try_from(total_sum / READING_ITERATIONS)
            .expect("The average of u16 readings fits inside a u16. This expect will never happen");
        let percent_moisture = self.calibration.percent(reading_avg);
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        Ok(Sample {
            ts: timestamp,
            percent: percent_moisture,
            millivolts: reading_avg,
        })
    }

    pub async fn run(&mut self, gauge: &MoistureGauge) {
        let mut failing = false;
        loop {
            let sample = match self.sample() {
                Ok(sample) => {
                    if failing {
                        log::info!("Moisture sensor recovered");
                        failing = false;
                    }

                    log::debug!(
                        "Read {} mV, which is {}% moisture",
                        sample.millivolts,
                        sample.percent
                    );
                    Some(sample)
                }
                Err(e) => {
                    if !failing {
                        log::warn!("Moisture sensor read failed: {e:#}");
                        failing = true;
                    }
                    None
                }
            };
            gauge.store(sample);
            Timer::after(self.refresh_interval).await;
        }
    }
}
