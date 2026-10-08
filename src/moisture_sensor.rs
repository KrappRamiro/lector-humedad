use esp_idf_svc::hal::{
    adc::{
        attenuation,
        oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
        AdcChannel,
    },
    gpio::ADCPin,
};

// TODO: Do this better, this kinda sucks
const DRY_READING: u16 = 2309;
const WET_READING: u16 = 780;

/// dry and wet reading are the lowest and highest readings the sensor can have.
fn percent_moisture_from_reading(raw_reading: u16, dry_reading: u16, wet_reading: u16) -> u16 {
    let clamped = raw_reading.clamp(wet_reading, dry_reading);

    let percent =
        (((dry_reading - clamped) as u32 * 100) / (dry_reading - wet_reading) as u32) as u16;
    log::debug!(
        "The percentage calculation from a raw reading of {} was {}",
        raw_reading,
        percent
    );

    percent
}

/// Domain knowledge needed to undestand this struct:
/// - `Pin` = the physical pin on the board.
/// - `AdcUnit` = the ADC hardware block inside the chip that does the actual
///   analog-to-digital conversion (like ADC1)
/// - `AdcChannel` = A Channel is a combination of pin + which ADC unit it's wired to.
/// - `AdcDriver` = a handle representing a claim over the whole ADC1 hardware block.
///   It doesn't know or care about any specific pin, it's just the ADC itself
/// - `AdcChannelDriver` = Like the AdcDriver, but for a Channel (The combination of pin + ADC)
///  
/// Now, to initialize a Moisture Sensor, we need two things:
/// - A pin where the sensor will be connected
/// - The ADC to use.
///
/// And do you remember what holds a combination of a pin and an ADC?
/// Yes, exactly, a Channel! Well, and `AdcChannel` to be exact.
/// That's why we will use a generic called `C` for that
pub struct MoistureSensor<'d, C: AdcChannel> {
    // If you read https://esp-rs.github.io/esp-idf-svc/esp_idf_svc/hal/adc/oneshot/struct.AdcChannelDriver.html
    // you will see this:
    //
    //      pub struct AdcChannelDriver<'d, C, M>
    //      where:
    //           C: AdcChannel,
    //           M: Borrow<AdcDriver<'d, <C as AdcChannel>::AdcUnit>>,
    //
    // of course, C is easy, its just the Adc Channel.
    // But M? M looks like black magic.
    // To help us understand what are we looking at,
    // we have the new() method in the same page, which looks like this:
    //
    //      pub fn new (
    //          adc: M,
    //          pin: impl ADCPin<AdcChannel = C> + 'd,
    //          config: &config::AdcChannelConfig,
    //      ) -> Result<Self, EspError>
    //
    // If we look at `adc` we have a clearer understand of what M is!
    // It's just the ADC driver. A borrowable one, so you could use T or &T
    //
    // So AdcChannelDriver becomes basically this:
    //      AdcChannelDriver <
    //          blank1: lifetime ,
    //          blank2: pin type ,
    //          blank3: an ADC driver
    //      >
    /// What constitutes as a dry reading , in terms of the value the ADC reports
    dry_reading: u16,
    /// What constitutes as a wet reading , in terms of the value the ADC reports
    wet_reading: u16,
    // To understand why this has this type, see the big-ass comment on top of the struct.
    // Just keep in mind it has to be a REFERENCE to the AdcDriver.
    // We should NEVER own the AdcDriver itself, because that would mean this sensor would have complete ownership of the ADC itself, which makes no fucking sense.
    adc_channel_driver: AdcChannelDriver<'d, C, &'d AdcDriver<'d, C::AdcUnit>>,
}

// This roughly translates to:
// "For any lifetime 'd and any channel type C -- as long as C qualifies as an AdcChannel -- here is a block of methods that work on a MoistureSensor built with that particular 'd and C
impl<'d, C: AdcChannel> MoistureSensor<'d, C> {
    pub fn new(
        // REMEMBER! C is an ADC Channel !
        //
        // pin's type is copied straight from AdcChannelDriver::new()'s own pin parameter
        // it means that pin type can be anything that implements ADCPin, as long as the AdcChannel the pin is physically connected to matches `C`
        pin: impl ADCPin<AdcChannel = C> + 'd,
        // Here, the adc type is an AdcDriver, and `C` holds which adc unit it is (adc1, adc2)
        adc: &'d AdcDriver<'d, C::AdcUnit>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            DRY_READING > WET_READING,
            "calibration invalid: dry reading must exceed wet reading"
        );
        // configuring pin to analog read, you can regulate the adc input voltage range depending on your need
        // for this example we use the attenuation of 11db which sets the input voltage range to around 0-3.6V
        let adc_config = AdcChannelConfig {
            attenuation: attenuation::DB_12,
            ..Default::default()
        };

        let adc_channel_driver = AdcChannelDriver::new(adc, pin, &adc_config)?;

        Ok(Self {
            dry_reading: DRY_READING,
            wet_reading: WET_READING,
            adc_channel_driver,
        })
    }
    pub fn read(&mut self) -> anyhow::Result<u16> {
        let mut total_sum: u32 = 0;
        const READING_ITERATIONS: u32 = 30;
        for _ in 0..READING_ITERATIONS {
            let reading = self.adc_channel_driver.read()?;
            total_sum += reading as u32;
        }
        let computed_reading: u16 = (total_sum / READING_ITERATIONS) as u16;
        let percent_moisture =
            percent_moisture_from_reading(computed_reading, self.dry_reading, self.wet_reading);
        Ok(percent_moisture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_reading_at_dry_bound_is_zero_percent() {
        let percent = percent_moisture_from_reading(DRY_READING, DRY_READING, WET_READING);
        assert_eq!(percent, 0);
    }

    #[test]
    fn raw_reading_at_wet_bound_is_hundred_percent() {
        let percent = percent_moisture_from_reading(WET_READING, DRY_READING, WET_READING);
        assert_eq!(percent, 100);
    }

    #[test]
    fn raw_reading_above_dry_bound_clamps_to_zero_percent() {
        let percent = percent_moisture_from_reading(DRY_READING + 500, DRY_READING, WET_READING);
        assert_eq!(percent, 0);
    }

    #[test]
    fn raw_reading_below_wet_bound_clamps_to_hundred_percent() {
        let percent = percent_moisture_from_reading(WET_READING - 500, DRY_READING, WET_READING);
        assert_eq!(percent, 100);
    }

    #[test]
    fn raw_reading_at_midpoint_is_fifty_percent() {
        // midpoint = (2309 + 780) / 2 = 1544 (integer division)
        // percent = ((2309 - 1544) * 100) / (2309 - 780) = 76500 / 1529 = 50 (floored)
        // If DRY_READING and WET_READING constants change some day, this test will go to shit
        let midpoint = (DRY_READING + WET_READING) / 2;
        let percent = percent_moisture_from_reading(midpoint, DRY_READING, WET_READING);
        assert_eq!(percent, 50);
    }
}
