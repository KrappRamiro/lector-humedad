use esp_idf_svc::hal::{
    adc::{
        AdcChannel, attenuation,
        oneshot::{AdcChannelDriver, AdcDriver, config::AdcChannelConfig},
    },
    gpio::ADCPin,
};
use fak_core::moisture::Calibration;

/// Domain knowledge needed to undestand this struct:
/// - `Pin` = the physical pin on the board.
/// - `AdcUnit` = the ADC hardware block inside the chip that does the actual
///   analog-to-digital conversion (like ADC1)
/// - `AdcChannel` = A Channel is a combination of pin + which ADC unit it's wired to.
/// - `AdcDriver` = a handle representing a claim over the whole ADC1 hardware block.
///   It doesn't know or care about any specific pin, it's just the ADC itself
/// - `AdcChannelDriver` = Like the `AdcDriver`, but for a Channel (The combination of pin + ADC)
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
    calibration: Calibration,

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
        calibration: Calibration,
        pin: impl ADCPin<AdcChannel = C> + 'd,
        // `C` holds which adc unit it is (adc1, adc2)
        adc: &'d AdcDriver<'d, C::AdcUnit>,
    ) -> anyhow::Result<Self> {
        // configuring pin to analog read, you can regulate the adc input voltage range depending on your need
        // for this example we use the attenuation of 11db which sets the input voltage range to around 0-3.6V
        let adc_config = AdcChannelConfig {
            attenuation: attenuation::DB_12,
            ..Default::default()
        };

        let adc_channel_driver = AdcChannelDriver::new(adc, pin, &adc_config)?;

        Ok(Self {
            calibration,
            adc_channel_driver,
        })
    }
    pub fn read(&mut self) -> anyhow::Result<u8> {
        const READING_ITERATIONS: u32 = 30;
        let mut total_sum: u32 = 0;
        for _ in 0..READING_ITERATIONS {
            let reading = self.adc_channel_driver.read()?;
            total_sum += u32::from(reading);
        }
        let reading_avg = u16::try_from(total_sum / READING_ITERATIONS)
            .expect("The average of u16 readings fits inside a u16. This expect will never happen");
        let percent_moisture = self.calibration.percent(reading_avg);

        log::debug!(
            "The percentage calculation from a reading of {reading_avg} mV was {percent_moisture}"
        );
        Ok(percent_moisture)
    }
}
