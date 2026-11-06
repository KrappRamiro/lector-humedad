#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Integer(i32),
    Bool(bool),
}

impl From<u8> for Value {
    fn from(v: u8) -> Self {
        Value::Integer(i32::from(v))
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

/// What will be published into `readings/<reading-id>`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub value: Value,
    /// Unix seconds, taken when the hardware sensor was read
    pub ts: u64,
}

/// Tracks some hardware sensor. Provides read access to the latest value of the sensor via the `latest` function
pub trait Gauge {
    fn reading_id(&self) -> &'static str;
    /// `None` until the first reading, or after a failed one
    fn latest(&self) -> Option<Reading>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn u8_becomes_integer() {
        assert_eq!(Value::from(42_u8), Value::Integer(42));
    }

    #[test]
    fn largest_u8_becomes_integer() {
        assert_eq!(Value::from(255_u8), Value::Integer(255));
    }

    #[test]
    fn bool_becomes_bool() {
        assert_eq!(Value::from(true), Value::Bool(true));
    }
}
