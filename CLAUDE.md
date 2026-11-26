# lector-humedad — learning context

This is a learning project: an ESP32-C3 device that reads soil moisture and
reacts to it. The user is not new to Rust but is rusty and relearning
embedded/ESP32-specific concepts.

How to teach (voice, lesson structure, the plan's Questions) lives in the
**Mentor** output style, `.claude/output-styles/mentor.md`, which
`.claude/settings.json` turns on. Edit teaching rules there, not here. This
file holds project facts and work rules.

## How to work in this project

- Verify ESP32/Rust API claims against real sources before stating them as
  fact — use the Context7 MCP tool (see below) or generate docs straight from
  the project's own locked dependencies with `cargo doc -p <crate> --no-deps
--open`, which reflects exactly what's compiled here, not just Context7 or
  general docs. Docs and code can drift out of sync with the resolved
  dependency versions of this project (e.g. Context7 pulled a `master` branch
  doc that turned out to be ahead of the code actually being compiled).
- Reference `std-training` examples (see below) for idiomatic style, but
  adapt rather than copy.

## Configuration checks belong in `build.rs`

The device runs unattended, without USB. A bad `cfg.toml` value that only
fails at runtime makes it restart in a loop in the field. So any rule a
`cfg.toml` value must follow (not empty, not the placeholder, a length limit
such as SSID at most 32 bytes or WPA2 password 8 to 63 characters, and so on)
is checked in `build.rs` with an `assert!`, so it fails on the build machine.

- Keep the runtime conversion (for example `try_into()` with `.context(...)`)
  when the types require it. It stays as a safety net.
- When a new `cfg.toml` key is added, or a new consumer puts a limit on an
  existing one, add its check to `build.rs` in the same change.
- Error messages state the rule: "cfg.toml: \`key\` can't ..." or
  "cfg.toml: \`key\` must ...".

## Hardware

**Board:** ESP32-C3 Super Mini (generic dev board). Single-core RISC-V, target `riscv32imc-esp-espidf`.

**Pinout notes specific to this board:**

- ADC-capable pins: GPIO0–GPIO4 only (ADC1, channels 0–4).
- GPIO9 is wired to the board's physical **BOOT** button — usable as a
  free debounced button input later (button stage), but it's also a
  strapping pin read at reset, so avoid external circuitry that could pull
  it low unexpectedly.
- GPIO8/GPIO9 are silkscreened "SDA/SCL" (I2C) — not needed for this
  project's current component list, but noted in case wiring ever gets
  confused with these labels.
- SPI (if ever needed): MISO=GPIO5, MOSI=GPIO6, SS=GPIO7, SCK=GPIO4.
- UART: RX=GPIO20, TX=GPIO21.
- Strapping pins to treat carefully: GPIO2, GPIO8, GPIO9.

## Components

1. **Capacitive Soil Moisture Sensor V2.0** (analog, in progress)
   - Output is regulated to 0–3.0V DC regardless of supply (per vendor
     datasheet), so either 3.3V or 5V VCC is electrically safe — this
     project uses 3.3V (simpler, matches the ESP32's own logic level).
   - Wiring: VCC → 3.3V, GND → GND, AOUT → GPIO0.
   - Rust side: `esp-idf-hal`'s ADC **oneshot** API (`AdcDriver` +
     `AdcChannelDriver`, `DB_12` attenuation for the full ~0–3.6V range).
   - Needs calibration by reading raw values dry vs. wet — don't hardcode a
     moisture threshold, the sensor output varies per unit.
2. **Display** — TM1637-driven 4-digit 7-segment module. TM1637 uses its
   own 2-wire CLK/DIO protocol, not I2C, don't reach for the I2C driver for this.

## Learning resources

- TM1637 library examples: https://github.com/JadKHaddad/tm1637/blob/main/examples/esp32/4-digits/src/bin/main.rs
- `std-training` repo (Espressif's official embedded Rust training):
  `/home/krapp/dev/fak/std-training`
  - `advanced/i2c-sensor-reading/examples/part_1.rs`,
    `advanced/button-interrupt/` — useful structural references even though
    this project's sensors aren't I2C.
  - `common/lib/rgb-led` — reference for the LED stage when we get there.
- Context7 MCP (`mcp__plugin_hm_context7__*`) — resolved library IDs already
  in use for this project:
  - `/esp-rs/esp-idf-hal` — peripheral drivers (GPIO, ADC, I2C, SPI, PWM).
  - `/esp-rs/esp-idf-svc` — higher-level services (WiFi, HTTP, MQTT, NVS,
    logging). Re-exports `esp-idf-hal` and `esp-idf-sys` as `esp_idf_svc::hal`
    and `esp_idf_svc::sys` — this project depends only on `esp-idf-svc`
    directly and reaches HAL code through that re-export, so imports go
    through `esp_idf_svc::hal::...`, not `esp_idf_hal::...` directly.
- Espressif Documentation MCP (`espressif-docs`, tool
  `search_espressif_sources`): searches official Espressif documentation,
  such as the ESP32-C3 datasheet and Technical Reference Manual, hardware
  design guidelines, and the ESP-IDF Programming Guide. Use it for questions
  about the C side and the hardware.
  - It covers only the newest ESP-IDF release. When that's newer than
    `ESP_IDF_VERSION` in `.cargo/config.toml`, check its answers against the
    local tree in `~/.espressif/esp-idf/<version>`, which is what this
    project compiles.
  - It doesn't cover the esp-rs crates or esp-mqtt (the `espressif/mqtt`
    component). Use Context7 or `cargo doc` for the crates, and the
    component's source for esp-mqtt.
  - Rate limit: 40 requests per hour, 200 per day. Search for the specific
    question instead of browsing with it.
