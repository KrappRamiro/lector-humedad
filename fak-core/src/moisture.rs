pub fn percent_moisture_from_reading(raw_reading: u16, dry_reading: u16, wet_reading: u16) -> u16 {
    //TODO: This needs to be redone
    let clamped = raw_reading.clamp(wet_reading, dry_reading);
    (((dry_reading - clamped) as u32 * 100) / (dry_reading - wet_reading) as u32) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    // The values the humidity sensor will report when dry and wet. Its not accurate. Could vary from sensor to sensor
    const DRY_READING: u16 = 2309;
    const WET_READING: u16 = 780;
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
