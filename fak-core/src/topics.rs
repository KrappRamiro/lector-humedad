use core::fmt::Display;
use core::fmt::Write;

/// Identifies one device via its WiFi MAC as lowercase hex, without any separators at all.
/// La especificación de MQTT 3.1.1 (sección 3.1.3.1) obliga al broker a aceptar client IDs de 1 a 23 bytes con solo 0-9a-zA-Z.
/// Cualquier otro carácter, incluido -, el broker puede aceptarlo pero no está obligado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceId(String);

impl DeviceId {
    #[must_use]
    pub fn from_mac(mac: [u8; 6]) -> Self {
        let mut id = String::with_capacity(12);
        for byte in mac {
            write!(id, "{byte:02x}").expect("Writing to an String cant fail");
        }
        Self(id)
    }

    /// The ID as a &str
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Topic where the device publishes `online` and its Last Will publishes `offline`
#[must_use]
pub fn availability_topic(device_id: &DeviceId) -> String {
    format!("fak/v1/devices/{device_id}/meta/availability")
}

/// Topic for one gauge's readings. `name` is the gauge's [`Gauge::name`](crate::gauge::Gauge::name)
#[must_use]
pub fn reading_topic(device_id: &DeviceId, name: &str) -> String {
    format!("fak/v1/devices/{device_id}/readings/{name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    // {:02x}
    //   │││
    //   ││└─ x  : hexadecimal en minúscula (X sería mayúscula)
    //   │└── 2  : ancho mínimo 2 caracteres
    //   └─── 0  : rellenar con ceros a la izquierda, no con espacios
    const FIXTURE_MAC: [u8; 6] = [0xa0, 0xb7, 0x65, 0x3f, 0x1c, 0x22];

    #[test]
    fn device_id_is_lowercase_hex_without_separators() {
        let id = DeviceId::from_mac(FIXTURE_MAC);
        assert_eq!(id.as_str(), "a0b7653f1c22");
    }

    #[test]
    fn device_id_keeps_leading_zeros() {
        let id = DeviceId::from_mac([0x0a, 0, 1, 2, 3, 4]);
        assert_eq!(id.as_str(), "0a0001020304");
    }

    #[test]
    fn device_id_displays_as_its_string() {
        let id = DeviceId::from_mac(FIXTURE_MAC);
        assert_eq!(format!("{id}"), id.as_str());
    }

    #[test]
    fn availability_topic_layout() {
        let id = DeviceId::from_mac(FIXTURE_MAC);
        assert_eq!(
            availability_topic(&id),
            "fak/v1/devices/a0b7653f1c22/meta/availability"
        );
    }

    #[test]
    fn reading_topic_layout() {
        let id = DeviceId::from_mac(FIXTURE_MAC);
        assert_eq!(
            reading_topic(&id, "moisture-01"),
            "fak/v1/devices/a0b7653f1c22/readings/moisture-01"
        );
    }
}
