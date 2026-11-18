use crate::gauge::{Reading, Value};

/// Last Will payload, published by the broker when the device disconnects abruptly without saying goodbye.
/// Could we say the device was a bad boy? (i should be revoken internet access)
pub const OFFLINE: &str = r#"{ "state": "offline" }"#;

/// Availability payload the device publishes after connecting.
/// `ts` is a timestamp in Unix seconds
#[must_use]
pub fn online(ts: u64) -> String {
    // donde {var} se usaria para mostrar el valor de la variable,
    // usar `{{  }}` se usa para escapar los {}, escribiendo literalmente `{ }`
    format!(r#"{{ "state": "online", "ts": {ts} }}"#)
}

/// Reading payload for a gauge's `readings/<name>` topic.
#[must_use]
pub fn reading(reading: &Reading) -> String {
    let value = match reading.value {
        Value::Integer(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        // ni se te ocurra poner _ =>
        // si haces esa huevada, el dia que agregues otro tipo de dato para Value::
        // te lo vas a estar morfando como un campeon.
    };
    format!(r#"{{ "value": {value}, "ts": {} }}"#, reading.ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    //  Mi cumple :)    TZ=America/Argentina/Buenos_Aires date -d '2004-04-23 11:00:00' +%s
    const FIXTURE_TS: u64 = 1_082_728_800;

    #[test]
    fn online_payload() {
        assert_eq!(
            online(FIXTURE_TS),
            r#"{ "state": "online", "ts": 1082728800 }"#
        );
    }

    #[test]
    fn offline_payload() {
        assert_eq!(OFFLINE, r#"{ "state": "offline" }"#);
    }

    #[test]
    fn integer_reading_payload() {
        let r = Reading {
            value: Value::Integer(42),
            ts: FIXTURE_TS,
        };
        assert_eq!(reading(&r), r#"{ "value": 42, "ts": 1082728800 }"#);
    }

    #[test]
    fn negative_integer_reading_payload() {
        let r = Reading {
            value: Value::Integer(-5),
            ts: FIXTURE_TS,
        };
        assert_eq!(reading(&r), r#"{ "value": -5, "ts": 1082728800 }"#);
    }

    #[test]
    fn bool_reading_payload() {
        let r = Reading {
            value: Value::Bool(true),
            ts: FIXTURE_TS,
        };
        assert_eq!(reading(&r), r#"{ "value": true, "ts": 1082728800 }"#);
    }
}
