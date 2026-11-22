//! Settings from `cfg.toml`, fixed at compile time.
//! `build.rs` checks the file and passes each value as an environment variable.

pub const WIFI_SSID: &str = env!("CFG_WIFI_SSID");
pub const WIFI_PASSWORD: &str = env!("CFG_WIFI_PASSWORD");
pub const MQTT_HOST: &str = env!("CFG_MQTT_HOST");
//TODO remove this expect on task 12
#[expect(dead_code, reason = "read by mqtt::start from Task 12 on")]
pub const MQTT_PASSWORD: &str = env!("CFG_MQTT_PASSWORD");
//TODO remove this expect on task 11
#[expect(dead_code, reason = "read by sntp::sync from Task 11 on")]
pub const SNTP_SERVER: &str = env!("CFG_SNTP_SERVER");
