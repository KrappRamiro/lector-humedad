// In this file, `mv` means millivolt.
// The moisture sensor returns a reading in millivolts, which the hardware ADC translates to a numeric value
use thiserror::Error;

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub struct Calibration {
    dry_mv: u16,
    wet_mv: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CalibrationError {
    #[error("dry reading ({dry_mv} mV) must be above wet reading ({wet_mv} mV)")]
    DryNotAboveWet { dry_mv: u16, wet_mv: u16 },
}

impl Calibration {
    /// Builds a calibration from the readings of a dry and a wet sensor.
    /// A Calibration ensures that the values you pass as bounds are correctly setup.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::DryNotAboveWet`] if `dry_mv` is not greater than `wet_mv`
    pub const fn new(dry_mv: u16, wet_mv: u16) -> Result<Calibration, CalibrationError> {
        if dry_mv <= wet_mv {
            return Err(CalibrationError::DryNotAboveWet { dry_mv, wet_mv });
        }
        Ok(Calibration { dry_mv, wet_mv })
    }

    /// Returns the percent of humidity from a raw `mv` reading.
    /// If you read [min-max_normalization](https://en.wikipedia.org/wiki/Feature_scaling#Rescaling_(min-max_normalization))
    /// you will find out this function is a Min-max rescaling of `mv` from `[wet_mv, dry_mv]` to `[100, 0]`
    /// Keep in mind a lower voltage means wetter soil
    ///
    /// # Panics
    ///
    /// In practice, never.
    /// `percent` is always a value between 0 and 100, so the conversion to u8 is guaranteed
    ///
    /// # Examples
    ///
    /// ```
    /// use fak_core::moisture::{Calibration, CalibrationError};
    ///
    /// let calibration = Calibration::new(2309, 780)?;
    /// assert_eq!(calibration.percent(780), 100);
    ///
    /// # // Why this Ok? See https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#using--in-doc-tests
    /// # Ok::<(), CalibrationError>(())
    /// ```
    #[must_use] // Why would you call this function otherwise?
    pub fn percent(&self, mv: u16) -> u8 {
        // Restricts the reading to be between the possible values
        let clamped = mv.clamp(self.wet_mv, self.dry_mv);
        let from_dry = u32::from(self.dry_mv - clamped);
        // The denominator from the formula (   max(x) - min(x)  )
        let span = u32::from(self.dry_mv - self.wet_mv);
        let percent = from_dry * 100 / span;
        u8::try_from(percent).expect(
            "percent is at most 100 because clamped >= wet_mv . This expect will never happen.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DRY_MV: u16 = 2309;
    const WET_MV: u16 = 780;
    const FIXTURE_CALIBRATION: Calibration = match Calibration::new(DRY_MV, WET_MV) {
        Ok(cal) => cal,
        Err(_) => {
            panic!("fixture calibration: the dry reading value must be above the wet reading value")
        }
    };

    #[test]
    fn percent_at_dry_bound_is_0() {
        assert_eq!(FIXTURE_CALIBRATION.percent(DRY_MV), 0);
    }

    #[test]
    fn percent_at_wet_bound_is_100() {
        assert_eq!(FIXTURE_CALIBRATION.percent(WET_MV), 100);
    }

    #[test]
    fn percent_above_dry_bound_clamps_to_0() {
        assert_eq!(FIXTURE_CALIBRATION.percent(DRY_MV + 500), 0);
    }

    #[test]
    fn percent_below_wet_bound_clamps_to_100() {
        assert_eq!(FIXTURE_CALIBRATION.percent(WET_MV - 500), 100);
    }

    #[test]
    fn percent_at_midpoint_is_50() {
        // (2309 + 780) / 2 = 1544; (2309 - 1544) * 100 / (2309 - 780) = 76500 / 1529 = 50
        let midpoint = u16::midpoint(DRY_MV, WET_MV);
        assert_eq!(FIXTURE_CALIBRATION.percent(midpoint), 50);
    }

    #[test]
    fn zero_millivolts_is_100_percent() {
        assert_eq!(FIXTURE_CALIBRATION.percent(0), 100);
    }

    #[test]
    fn new_rejects_dry_below_wet() {
        assert_eq!(
            Calibration::new(780, 2309),
            Err(CalibrationError::DryNotAboveWet {
                dry_mv: 780,
                wet_mv: 2309
            })
        );
    }

    #[test]
    fn new_rejects_equal_readings() {
        assert_eq!(
            Calibration::new(1000, 1000),
            Err(CalibrationError::DryNotAboveWet {
                dry_mv: 1000,
                wet_mv: 1000
            })
        );
    }

    #[test]
    fn error_message_names_both_values() {
        let error = Calibration::new(780, 2309).unwrap_err();
        assert_eq!(
            error.to_string(),
            "dry reading (780 mV) must be above wet reading (2309 mV)"
        );
    }
}
