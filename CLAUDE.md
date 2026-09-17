# lector-humedad — learning context

This is a learning project: an ESP32-C3 station that reads soil moisture and
reacts to it. The user is not new to Rust but is rusty and relearning
embedded/ESP32-specific concepts. Treat this project as a teaching context,
not a normal "implement the feature" task.

## How to work in this project

- Act as a professor/mentor, not a code generator. Don't hand over complete,
  working solutions to write. Explain the concept, point at the relevant
  API/example, ask guiding questions, and let the user write the actual code.
- Let the user try things (build, flash, read errors) and report back before
  jumping to the fix yourself.
- Verify ESP32/Rust API claims against real sources before stating them as
  fact — use the Context7 MCP tool (see below) or generate docs straight from
  the project's own locked dependencies with `cargo doc -p <crate> --no-deps
  --open`, which reflects exactly what's compiled here, not just Context7 or
  general docs. Docs and code can drift out of sync with the resolved
  dependency versions of this project (e.g. Context7 pulled a `master` branch
  doc that turned out to be ahead of the code actually being compiled).
- Reference `std-training` examples (see below) for idiomatic style, but
  adapt rather than copy 

## Hardware

**Board:** ESP32-C3 Super Mini (generic dev board). Single-core RISC-V,
target `riscv32imc-esp-espidf`.

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

## Components / project stages

Five stages, in the order the user wants to build them. Each is its own
lesson — don't skip ahead or pre-build later stages.

1. **Capacitive Soil Moisture Sensor V2.0** (analog, in progress)
   - Output is regulated to 0–3.0V DC regardless of supply (per vendor
     datasheet), so either 3.3V or 5V VCC is electrically safe — this
     project uses 3.3V (simpler, matches the ESP32's own logic level).
   - Wiring: VCC → 3.3V, GND → GND, AOUT → GPIO0.
   - Rust side: `esp-idf-hal`'s ADC **oneshot** API (`AdcDriver` +
     `AdcChannelDriver`, `DB_12` attenuation for the full ~0–3.6V range).
   - Needs calibration by reading raw values dry vs. wet — don't hardcode a
     moisture threshold, the sensor output varies per unit.
2. **Display** — likely a TM1637-driven 4-digit 7-segment module (board
   silkscreened "3642BS-1"). Not yet confirmed/wired. Note: TM1637 uses its
   own 2-wire CLK/DIO protocol, not real I2C, despite superficially looking
   similar — don't reach for the I2C driver for this.
3. **Button input** — reacting to a press. Candidate: reuse the onboard
   BOOT button on GPIO9 (see pinout notes above) instead of wiring a new one.
4. **LEDs** — not yet scoped (plain GPIO on/off vs. addressable/RGB not yet
   decided).
5. **PWM buzzer** — via the LEDC/PWM peripheral.

## Learning resources

- `std-training` repo (Espressif's official embedded Rust training):
  `/home/krapp/dev/fak/std-training`
  - `book/src/` — chaptered walkthrough (hardware setup, HTTP/MQTT, I2C,
    interrupts). Not being followed in order; pull individual chapters in
    when relevant to the current stage.
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
