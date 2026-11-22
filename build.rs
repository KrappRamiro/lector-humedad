// build.rs runs on the machine that builds the code, before the firmware is compiled.
// It is the only reader of cfg.toml: it checks the file and hands each value to the
// compiler as an environment variable, which src/config.rs reads with env!().

use serde::Deserialize;
use std::{fs, io};

/// The SNTP server used when `cfg.toml` doesn't set `sntp_server`.
const DEFAULT_SNTP_SERVER: &str = "0.pool.ntp.org";

/// The whole `cfg.toml` file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CfgFile {
    #[serde(rename = "lector-humedad")]
    lector_humedad: Config,
}

/// The `[lector-humedad]` table.
/// CI writes a `cfg.toml` with these keys in `rust_ci.yml`, so keep it in sync.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    wifi_ssid: String,
    wifi_password: String,
    mqtt_host: String,
    mqtt_password: String,
    sntp_server: Option<String>,
}

/// One value from `cfg.toml` that `build.rs` checks and passes to the firmware.
struct Setting<'a> {
    /// The key in `cfg.toml`. `src/config.rs` reads it as `CFG_<KEY>`. For example: `CFG_WIFI_SSID`.
    key: &'static str,
    value: &'a str,
    /// The value `cfg.toml.example` gives this key, if it has one.
    placeholder: Option<&'static str>,
}

fn main() {
    // Run this script again whenever cfg.toml changes.
    // See https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
    println!("cargo::rerun-if-changed=cfg.toml");

    let text = match fs::read_to_string("cfg.toml") {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            panic!("cfg.toml is missing. Copy cfg.toml.example to cfg.toml and fill it in.")
        }
        Err(e) => panic!("cfg.toml can't be read: {e}"),
    };

    let config = toml::from_str::<CfgFile>(&text)
        .unwrap_or_else(|e| panic!("cfg.toml is invalid:\n{e}"))
        .lector_humedad;

    // The WiFi driver stores the SSID in 32 bytes, and a WPA2 passphrase has 8 to 63
    // characters. Checking here turns a bad value into a build error instead of a boot
    // loop on the device. len() counts bytes, the same unit as those limits.
    assert!(
        config.wifi_ssid.len() <= 32,
        "cfg.toml: `wifi_ssid` can't be longer than 32 bytes"
    );
    assert!(
        (8..=63).contains(&config.wifi_password.len()),
        "cfg.toml: `wifi_password` must have 8 to 63 characters"
    );

    let sntp_server = config
        .sntp_server
        .unwrap_or_else(|| DEFAULT_SNTP_SERVER.to_owned());

    let settings = [
        Setting {
            key: "wifi_ssid",
            value: &config.wifi_ssid,
            placeholder: Some("your-ssid"),
        },
        Setting {
            key: "wifi_password",
            value: &config.wifi_password,
            placeholder: Some("your-wifi-password"),
        },
        Setting {
            key: "mqtt_host",
            value: &config.mqtt_host,
            placeholder: Some("your-mqtt-host"),
        },
        Setting {
            key: "mqtt_password",
            value: &config.mqtt_password,
            placeholder: Some("your-mqtt-password"),
        },
        Setting {
            key: "sntp_server",
            value: &sntp_server,
            placeholder: None,
        },
    ];

    for setting in settings {
        let Setting {
            key,
            value,
            placeholder,
        } = setting;
        assert!(!value.is_empty(), "cfg.toml: `{key}` is empty");
        if let Some(placeholder) = placeholder {
            assert!(
                value != placeholder,
                "cfg.toml: `{key}` still has the placeholder {placeholder:?}"
            );
        }
        // A cargo:: instruction ends at the line break, so the value can't contain one.
        assert!(
            !value.contains(['\n', '\r']),
            "cfg.toml: `{key}` can't contain a line break"
        );
        let variable = format!("CFG_{}", key.to_uppercase());
        // Sets the environment variable.
        // See https://docs.rs/cargo-build/latest/cargo_build/fn.rustc_env.html
        println!("cargo::rustc-env={variable}={value}");
    }

    embuild::espidf::sysenv::output();
}
