use crate::gauge::{Reading, Value};

/// Readout is the logical representation of what a screen can show.
/// - `Number`: Represents a number of type N that can be shown
/// - `TooHigh`: Represents that the number that you tried to show was too high for the screen you are trying to show it in.
/// - `TooLow`: Represents that the number that you tried to show was too low for the screen you are trying to show it in.
/// - `On`: Represents a boolean true state
/// - `Off`: Represents a boolean false state
/// - `NoReading`: Represents that there is no data to show on the screen
///
/// The function `readout_for` is the one responsible for creating Readouts
/// It is not recommendable to create readouts manually. Why? Because if you had a 4 digit screen and wanted to create a Readout for the number `12345`, `readout_for` would return `TooHigh`, _as it should_. But if you made the mistake of making `Readout::Number(12345)` manually, no one could stop you
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Readout<N> {
    Number(N),
    TooHigh,
    TooLow,
    On,
    Off,
    NoReading,
}

/// Screens that can show numbers (like a TM1637 screen) will implement this Trait.
/// For example, if you had this struct:
/// ```
/// use fak_core::readout::NumericScreen;
/// struct MySingleDigitScreen;
///
/// // You would implement the trait like this, to show that the screen can show numbers that are a u8, and only from 0 to 9
///
/// impl NumericScreen for MySingleDigitScreen {
///     type Number = u8;
///     const DRAWABLE_RANGE: std::ops::RangeInclusive<Self::Number> = 0..=9;
/// }
/// ```
pub trait NumericScreen {
    /// The type of number this screen will hold.
    /// Number must be a primitive integer type (like u8, i16, i32)
    /// Do not try to use `NonZero` numbers
    type Number: TryFrom<i32> + PartialOrd + Copy;
    const DRAWABLE_RANGE: std::ops::RangeInclusive<Self::Number>;
}

/// The numbers a four-digit seven-segment display can draw: three digits and a minus sign
/// on the negative side, four digits on the positive side.
pub const FOUR_DIGITS: std::ops::RangeInclusive<i16> = -999..=9999;

/// Takes `reading` and returns the matching `Readout`
/// - If the reading has no value, it returns `Readout::NoReading`
/// - If it's `Value::Bool(true)`, it returns `Readout::On`
/// - If it's `Value::Bool(false)`, it returns `Readout::Off`
/// - If it's a number, checks if it fits in the given screen `S` :
///   - If it does, returns a `Readout::Number(n)` that the screen can use to display the number.
///   - If it does not fit, returns a `Readout::TooHigh` or `Readout::TooLow` that represents whether the number `n` is too high or too low to fit in the given screen `S`
#[must_use]
pub fn readout_for<S: NumericScreen>(reading: Option<Reading>) -> Readout<S::Number> {
    let Some(Reading { value, .. }) = reading else {
        return Readout::NoReading;
    };

    match value {
        Value::Bool(true) => Readout::On,
        Value::Bool(false) => Readout::Off,
        Value::Integer(n) => match S::Number::try_from(n) {
            Ok(x) if S::DRAWABLE_RANGE.contains(&x) => Readout::Number(x),
            Ok(x) if x > *S::DRAWABLE_RANGE.end() => Readout::TooHigh,
            Err(_) if n > 0 => Readout::TooHigh,
            Ok(_) | Err(_) => Readout::TooLow,
        },
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // -- Fixtures -- //
    struct FixtureDigit;

    impl NumericScreen for FixtureDigit {
        type Number = u8;
        const DRAWABLE_RANGE: std::ops::RangeInclusive<Self::Number> = 0..=9;
    }

    struct FixtureFourDigits;

    impl NumericScreen for FixtureFourDigits {
        type Number = i16;
        const DRAWABLE_RANGE: std::ops::RangeInclusive<Self::Number> = FOUR_DIGITS;
    }

    struct FixtureTenToTwenty;

    impl NumericScreen for FixtureTenToTwenty {
        type Number = u8;
        const DRAWABLE_RANGE: std::ops::RangeInclusive<Self::Number> = 10..=20;
    }
    // --- //

    fn helper_i32_to_reading(n: i32) -> Reading {
        Reading {
            value: Value::Integer(n),
            ts: 0,
        }
    }

    fn helper_bool_to_reading(b: bool) -> Reading {
        Reading {
            value: Value::Bool(b),
            ts: 0,
        }
    }

    #[test]
    fn no_reading_shows_dashes() {
        assert_eq!(Readout::NoReading, readout_for::<FixtureDigit>(None));
    }

    #[test]
    fn true_shows_on() {
        assert_eq!(
            Readout::On,
            readout_for::<FixtureDigit>(Some(helper_bool_to_reading(true)))
        );
    }

    #[test]
    fn false_shows_off() {
        assert_eq!(
            Readout::Off,
            readout_for::<FixtureDigit>(Some(helper_bool_to_reading(false)))
        );
    }

    #[test]
    fn zero_is_shown() {
        assert_eq!(
            Readout::Number(0),
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(0)))
        );
    }

    #[test]
    fn largest_four_digit_value_is_shown() {
        assert_eq!(
            Readout::Number(9999),
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(9999)))
        );
    }

    #[test]
    fn smallest_negative_value_is_shown() {
        assert_eq!(
            Readout::Number(-999),
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(-999)))
        );
    }

    #[test]
    fn five_digit_value_is_too_high() {
        assert_eq!(
            Readout::TooHigh,
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(10000)))
        );
    }

    #[test]
    fn four_digit_negative_is_too_low() {
        assert_eq!(
            Readout::TooLow,
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(-1000)))
        );
    }

    #[test]
    fn value_beyond_i16_is_too_high() {
        assert_eq!(
            Readout::TooHigh,
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(70000)))
        );
    }

    #[test]
    fn value_beyond_i16_is_too_low() {
        assert_eq!(
            Readout::TooLow,
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(-70000)))
        );
    }

    #[test]
    fn largest_i32_is_too_high() {
        assert_eq!(
            Readout::TooHigh,
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(i32::MAX)))
        );
    }

    #[test]
    fn smallest_i32_is_too_low() {
        assert_eq!(
            Readout::TooLow,
            readout_for::<FixtureFourDigits>(Some(helper_i32_to_reading(i32::MIN)))
        );
    }

    #[test]
    fn fits_inside_range_is_number() {
        assert_eq!(
            Readout::Number(5),
            readout_for::<FixtureDigit>(Some(helper_i32_to_reading(5)))
        );
    }

    #[test]
    fn above_range_is_too_high() {
        assert_eq!(
            Readout::TooHigh,
            readout_for::<FixtureDigit>(Some(helper_i32_to_reading(10)))
        );
    }

    #[test]
    fn below_range_is_too_low() {
        assert_eq!(
            Readout::TooLow,
            readout_for::<FixtureTenToTwenty>(Some(helper_i32_to_reading(3)))
        );
    }

    #[test]
    fn above_number_type_is_too_high() {
        assert_eq!(
            Readout::TooHigh,
            readout_for::<FixtureDigit>(Some(helper_i32_to_reading(300)))
        );
    }

    #[test]
    fn below_number_type_is_too_low() {
        assert_eq!(
            Readout::TooLow,
            readout_for::<FixtureDigit>(Some(helper_i32_to_reading(-5)))
        );
    }

    #[test]
    fn range_start_is_shown() {
        assert_eq!(
            Readout::Number(10),
            readout_for::<FixtureTenToTwenty>(Some(helper_i32_to_reading(10)))
        );
    }

    #[test]
    fn range_end_is_shown() {
        assert_eq!(
            Readout::Number(20),
            readout_for::<FixtureTenToTwenty>(Some(helper_i32_to_reading(20)))
        );
    }
}
