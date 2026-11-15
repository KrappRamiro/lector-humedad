# MQTT Integration Implementation Plan

> **How this plan is executed:** the project owner writes all the code.
> Claude acts as a mentor: before each task it explains the task's concepts,
> answers questions, and after each task it reviews the owner's diff. Claude
> does not write the code. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The board connects to WiFi, syncs its clock, and publishes its
moisture reading to the `fak` MQTT broker every 60 seconds, with
availability, a Last Will, typed errors, and a logging policy. When WiFi,
SNTP, or MQTT setup fails, the display shows an error code.

**Architecture:** Pure logic (calibration, value types, topics, payloads,
display and WiFi decisions) lives in `fak-core` and is built test-first on
the development machine. The firmware runs its loops as boxed futures under
one `block_on(select_array(..))`. All MQTT client calls happen on one
dedicated `std::thread`, fed through a `std::sync::mpsc::sync_channel(8)`.

**Tech Stack:** `esp-idf-svc` 0.53.0 (with `esp-idf-hal` 0.47.0 and
`esp-idf-sys` 0.38.1, all from crates.io), ESP-IDF v6.1, `espressif/mqtt` 1.1.0, `embassy-time` 0.5.1, `embassy-sync` 0.8.0,
`embassy-futures` 0.1.2, `toml-cfg`, `anyhow`, `log`, `tm1637-embedded-hal`
0.5.3.

**Spec:** `docs/superpowers/specs/2026-10-15-mqtt-integration-design.md`
(section numbers below refer to it). Requires
`docs/superpowers/plans/2026-10-17-workspace-split.md` to be done first.

## Global Constraints

- HAL imports go through `esp_idf_svc::hal::...`, never `esp_idf_hal::...`.
- `fak-core`: no `esp-idf-*`, `embassy-*`, or `log` dependency;
  `#![forbid(unsafe_code)]`; public items satisfy pedantic from the start
  (`#[must_use]`, `# Errors`, `# Panics`).
- `fak-core` errors are typed `enum`s implementing `core::error::Error`; the
  firmware uses `anyhow::Result` and adds `.context(..)` to every setup step.
- No silent `as` casts: widen with `From`, narrow with
  `try_from(..).expect("<why it can't fail>")`.
- No bare `unwrap()` outside tests. `expect` and `unreachable!` only for
  broken invariants.
- Durations: `embassy_time::Duration` for async timing and this design's
  timeouts; `core::time::Duration` for ESP-IDF configuration and in
  `fak-core`.
- Log levels follow section 6 ("Levels"); log changes, not repetitions; never
  log `wifi_password` or `mqtt_password`.
- Every `unsafe` block has a `// SAFETY:` comment naming what it relies on.
- Gate after every task: `devenv test --no-tui` passes. It runs
  `cargo fmt --check`, `cargo test-core`, clippy on the whole workspace with
  `-D warnings`, and a release build (the cargo commands other than `fmt`
  use `--locked`). CI runs the same
  command. Pedantic is advice (workspace spec, section 7): run it with
  `clippy-pedantic`. Then `/code-review` on the diff, then commit.
- The firmware is a binary crate, so the gate rejects code nothing uses yet
  (`dead_code`). Add an item in the task that first uses it. Where a task
  must leave something unused for a later task, it says so and uses
  `#[expect(dead_code, reason = "..")]`, which the later task removes.
- Each new firmware module is added to `DEBUG_TARGETS` (from Task 6 on).

## Review Focus

Conditions that no automated test exercises, most likely first. Each has a
board check in the task named.

1. **Wrong MQTT password in `cfg.toml`.** Expected: the board doesn't look
   healthy. `wait_connected` times out after 30 s, the esp-mqtt log says
   why, and the board restarts every ~40 s. (Task 12, Step 9.)
2. **The broker goes away for minutes, then comes back.** Expected: no
   restart; readings older than 30 s are dropped, `online` is republished on
   reconnect, and readings resume. The sensor and display keep updating the
   whole time. (Task 13, Step 7.)
3. **The WiFi access point reboots.** Expected: a drop shorter than 60 s
   recovers with one `warn` and one `info`; a longer one restarts the board.
   (Task 10, Step 7.)
4. **The sensor's AOUT wire comes loose.** Expected, per the current design:
   the reading isn't distinguishable from very wet soil (low millivolts give
   100 %). The plan pins today's behavior with a test (`zero_millivolts_is_hundred_percent`,
   Task 2) and a board check, so a future "sensor missing" feature starts
   from a known point. (Task 5, Step 8.)
5. **`cfg.toml` copied from the example and not edited.** Expected: the build
   fails with a message naming the file, not a board that boot-loops.
   (Task 7, Step 5.)
6. **A setup failure with nobody watching the log.** Expected: the display
   shows the failed step's code (`E-01` WiFi, `E-02` SNTP, `E-03` MQTT) for
   the whole 10-second pause, then `----` while setup runs again.
   (Task 14, Step 8.)

---

### Task 1: Use the crates.io releases of the ESP-IDF crates (done)

**Files:** Modify `Cargo.toml` (remove `[patch.crates-io]`), `Cargo.lock`.

The original task pinned `rev` on the three git dependencies. Before it
started, upstream merged `esp-idf-sys`, `esp-idf-hal`, and `esp-idf-svc` into
the [esp-rs/esp-idf](https://github.com/esp-rs/esp-idf) monorepo and
archived the old repositories. The releases that carry the v6 support
(`esp-idf-svc` 0.53.0, `esp-idf-hal` 0.47.0, `esp-idf-sys` 0.38.1) were
already on crates.io, so the patch was removed instead. A registry version
never changes once published, so `Cargo.lock` pins it without a `rev`.

**Concepts:** what `[patch.crates-io]` does; git dependencies without `rev`
follow a branch; registry versions are immutable and checksummed.

- [x] **Step 1:** Remove `[patch.crates-io]`; `cargo update -p esp-idf-svc
  -p esp-idf-hal -p esp-idf-sys`. `Cargo.lock` changes only those three
  `source` lines (plus their `checksum`); `esp-idf-sys` goes 0.38.0 to
  0.38.1. `esp-idf-hal` and `esp-idf-svc` are byte-identical to the
  previously locked commits. `esp-idf-sys` 0.38.1 only narrows some libc
  checks and adds Mbed TLS headers to `bindings.h`.
- [x] **Step 2:** Gate, board check, commit:
  `build: use crates.io releases of the esp-idf crates`.

---

### Task 2: `Calibration` in `fak-core` (test-first, done)

**Files:**
- Modify: `fak-core/src/moisture.rs` (replace `percent_moisture_from_reading`)
- Modify: `src/moisture_sensor.rs` (take a `Calibration`; drop the constants
  and the `ensure!`; fix the two comments from section 4)
- Modify: `src/main.rs` (add `MOISTURE_SENSOR_01_CALIBRATION`)

**Interfaces (section 4):**
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Calibration { dry_mv: u16, wet_mv: u16 }`
  - `pub const fn Calibration::new(dry_mv: u16, wet_mv: u16) -> Result<Calibration, CalibrationError>`
  - `#[must_use] pub fn Calibration::percent(&self, millivolts: u16) -> u8`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum CalibrationError { DryNotAboveWet { dry_mv: u16, wet_mv: u16 } }`
    with `Display` and `core::error::Error`.
  - `MoistureSensor::new(calibration: Calibration, pin, adc)` (the refresh
    interval arrives in Task 5).

**Concepts first:** "parse, don't validate" (a value that can only exist if
valid); private fields; `const fn` and computing a `const` at build time;
deriving `Display` and `Error` with `thiserror`; `u32::from` and `u8::try_from`;
`# Errors` and `# Panics` doc sections; tests that return `Result` so `?`
works inside them; doc examples as tests.

**Questions:** Why can't `percent` divide by zero? What would happen at build
time if `MOISTURE_SENSOR_01_CALIBRATION` used `new(780, 2309)`?

- [x] **Step 1: Write the failing tests** in `fak-core/src/moisture.rs`.
  Tests return `Result<(), CalibrationError>` and build with
  `Calibration::new(2309, 780)?`:

  | Test | Assertion |
  |---|---|
  | `percent_at_dry_bound_is_zero` | `percent(2309) == 0` |
  | `percent_at_wet_bound_is_hundred` | `percent(780) == 100` |
  | `percent_above_dry_bound_clamps_to_zero` | `percent(2809) == 0` |
  | `percent_below_wet_bound_clamps_to_hundred` | `percent(280) == 100` |
  | `percent_at_midpoint_is_fifty` | `percent(1544) == 50` |
  | `zero_millivolts_is_hundred_percent` | `percent(0) == 100` (Review Focus 4) |
  | `new_rejects_dry_below_wet` | `Calibration::new(780, 2309) == Err(CalibrationError::DryNotAboveWet { dry_mv: 780, wet_mv: 2309 })` |
  | `new_rejects_equal_readings` | `Calibration::new(1000, 1000)` is that same variant with `1000, 1000` |
  | `error_message_names_both_values` | `.to_string() == "dry reading (780 mV) must be above wet reading (2309 mV)"` |

  Plus an `# Examples` doc block on `percent` showing `new(2309, 780)` and
  `percent(780) == 100`.
- [x] **Step 2:** `cargo test-core`. Expected: compile errors (nothing is
  defined yet).
- [x] **Step 3: Implement** `Calibration`, `CalibrationError`, and their
  impls. `percent` clamps to `wet_mv..=dry_mv`, computes in `u32`, narrows
  with `u8::try_from(..).expect(..)`. Delete
  `percent_moisture_from_reading` and its 5 old tests (the new ones replace
  them).
- [x] **Step 4:** `cargo test-core`. Expected: 9 unit tests and 1 doc-test
  pass.
- [x] **Step 5: Firmware.** Add `MOISTURE_SENSOR_01_CALIBRATION` to `main.rs`, as a
  `match` with `panic!` (section 4). `MoistureSensor::new` takes it.
- [ ] **Step 6: Prove the build-time check.** (skipped) Temporarily change it to
  `new(780, 2309)` and run `cargo build`. Expected: a compile error quoting
  your panic message. Revert.
- [x] **Step 7: Board.** Same percentage as before.
- [x] **Step 8:** Gate (pedantic should now be clean for `moisture.rs`),
  review, commit: `feat(core): add validated Calibration`.

---

### Task 3: `Value`, `Reading`, `Gauge` (test-first, done)

**Files:** Create `fak-core/src/gauge.rs`; modify `fak-core/src/lib.rs`.

**Interfaces (section 4, `gauge.rs`):**
- Produces: `pub enum Value { Integer(i32), Bool(bool) }`,
  `impl From<u8> for Value`, `impl From<bool> for Value`,
  `pub struct Reading { pub value: Value, pub ts: u64 }`, and
  `pub trait Gauge { fn reading_id(&self) -> &'static str; fn latest(&self) -> Option<Reading>; }`.
  Both types derive `Debug, Clone, Copy, PartialEq, Eq`.

**Concepts first:** closed enums and exhaustive `match`; `From` and
`.into()`; traits and `&dyn Trait` (trait objects); why the trait has no
associated type (E0191, section 4).

- [x] **Step 1: Failing tests:** `Value::from(42_u8) == Value::Integer(42)`,
  `Value::from(255_u8) == Value::Integer(255)`,
  `Value::from(true) == Value::Bool(true)`.
- [x] **Step 2:** `cargo test-core`: fails to compile.
- [x] **Step 3: Implement** the types, impls, and trait.
- [x] **Step 4:** `cargo test-core`: passes.
- [x] **Step 5:** Gate, review, commit: `feat(core): add Value, Reading and Gauge`.

---

### Task 4: `readout::readout_for` (test-first, done)

**Files:** Create `fak-core/src/readout.rs`; modify `fak-core/src/lib.rs`.

**Interfaces:** Consumes `Reading`, `Value` (Task 3). Produces (section 4,
`fak-core/src/readout.rs`):
- `pub enum Readout<N> { Number(N), TooHigh, TooLow, On, Off, NoReading }`,
  with the doc comment saying to build it with `readout_for`.
- `pub trait NumericScreen { type Number: TryFrom<i32> + PartialOrd + Copy; const DRAWABLE_RANGE: RangeInclusive<Self::Number>; }`
- `pub const FOUR_DIGITS: RangeInclusive<i16> = -999..=9999;`
- `#[must_use] pub fn readout_for<S: NumericScreen>(reading: Option<Reading>) -> Readout<S::Number>`

**Concepts first:** a generic enum (`Readout<N>`); a trait with an associated
type that has bounds, and an associated const whose type uses it
(`RangeInclusive<Self::Number>`); calling a generic function with turbofish
(`readout_for::<FixtureFourDigits>`); `let ... else`; literal patterns
(`Value::Bool(true)`); `S::Number::try_from(v)` matched on `Ok`/`Err` with
guards, and why guarded arms don't count toward exhaustiveness; why the range
check is a guard and not a range pattern (E0308, E0158); why `TooHigh`
compares with `DRAWABLE_RANGE.end()` for `Ok` but with zero for `Err`; the blanket
`TryFrom` with `Error = Infallible`; why the bounds are -999 and 9999
(section 4).

**Questions:** Why does a failed `try_from` mean the value is outside the
range, and how does its sign say which side? Why can't the test screen
`FixtureTenToTwenty` use `Number = i16` and still cover every arm? What can still go
wrong with `Readout::<i16>::Number(12345)`, and what stops it today?

- [x] **Step 1: Failing tests** (any `ts`). Two test-only screens in the test
  module: `FixtureFourDigits` (`Number = i16`, `DRAWABLE_RANGE = FOUR_DIGITS`) and
  `FixtureTenToTwenty` (`Number = u8`, `DRAWABLE_RANGE = 10..=20`).

  | Test | Screen | Input | Expected |
  |---|---|---|---|
  | `no_reading_shows_dashes` | `FixtureFourDigits` | `None` | `NoReading` |
  | `zero_is_shown` | `FixtureFourDigits` | `Integer(0)` | `Number(0)` |
  | `largest_four_digit_value_is_shown` | `FixtureFourDigits` | `Integer(9999)` | `Number(9999)` |
  | `smallest_negative_value_is_shown` | `FixtureFourDigits` | `Integer(-999)` | `Number(-999)` |
  | `five_digit_value_is_too_high` | `FixtureFourDigits` | `Integer(10000)` | `TooHigh` |
  | `four_digit_negative_is_too_low` | `FixtureFourDigits` | `Integer(-1000)` | `TooLow` |
  | `value_beyond_i16_is_too_high` | `FixtureFourDigits` | `Integer(70000)` | `TooHigh` |
  | `value_beyond_i16_is_too_low` | `FixtureFourDigits` | `Integer(-70000)` | `TooLow` |
  | `largest_i32_is_too_high` | `FixtureFourDigits` | `Integer(i32::MAX)` | `TooHigh` |
  | `smallest_i32_is_too_low` | `FixtureFourDigits` | `Integer(i32::MIN)` | `TooLow` |
  | `true_shows_on` | `FixtureFourDigits` | `Bool(true)` | `On` |
  | `false_shows_off` | `FixtureFourDigits` | `Bool(false)` | `Off` |
  | `below_range_is_too_low` | `FixtureTenToTwenty` | `Integer(5)` | `TooLow` |
  | `range_start_is_shown` | `FixtureTenToTwenty` | `Integer(10)` | `Number(10)` |
  | `range_end_is_shown` | `FixtureTenToTwenty` | `Integer(20)` | `Number(20)` |
  | `above_range_is_too_high` | `FixtureTenToTwenty` | `Integer(25)` | `TooHigh` |
  | `below_number_type_is_too_low` | `FixtureTenToTwenty` | `Integer(-5)` | `TooLow` |
  | `above_number_type_is_too_high` | `FixtureTenToTwenty` | `Integer(300)` | `TooHigh` |

- [x] **Step 2:** `cargo test-core`: fails to compile.
- [x] **Step 3: Implement** `Readout`, `NumericScreen`, `FOUR_DIGITS`, and
  `readout_for`, and register the module in `lib.rs`.
- [x] **Step 4:** `cargo test-core`: passes.
- [x] **Step 5:** Gate, review, commit: `feat(core): add display readout rule`.

---

### Task 5: Sensor and gauge split, display loop, `select_array`

**Files:**
- Modify: `src/moisture_sensor.rs` (`Sample`, `MoistureGauge`,
  `MoistureSensor::run`, `sample`)
- Create: `src/display.rs` (`Tm1637Display`, `Draw`, `Tm1637Screen`,
  `display_loop`)
- Modify: `src/main.rs` (task array, `TASK_NAMES`, remove `sensor_display`
  and `TM1637DisplayT`)
- Modify: `Cargo.toml` (add `embassy-futures`, already locked at 0.1.2)

**Interfaces (section 4):**
- Consumes: `Calibration` (Task 2), `Gauge`, `Reading`, `Value` (Task 3),
  `Readout`, `NumericScreen`, `FOUR_DIGITS`, `readout_for` (Task 4).
- Produces:
  - `pub struct MoistureGauge { reading_id: &'static str, latest: Cell<Option<Sample>> }`,
    `MoistureGauge::new(reading_id: &'static str) -> Self`, private
    `store(&self, sample: Option<Sample>)`, and `impl Gauge for MoistureGauge`.
  - `MoistureSensor::new(calibration, pin, adc, refresh_interval: embassy_time::Duration) -> anyhow::Result<Self>`
  - `pub async fn MoistureSensor::run(&mut self, gauge: &MoistureGauge)`;
    private `fn sample(&mut self) -> anyhow::Result<Sample>`.
  - `pub type Tm1637Display` (section 4).
  - `pub trait Draw: NumericScreen { type Error: std::fmt::Debug; async fn show(&mut self, readout: Readout<Self::Number>) -> Result<(), Self::Error>; }`
  - `pub struct Tm1637Screen(Tm1637Display)` with
    `pub async fn init(display: Tm1637Display) -> Result<Self, tm1637_embedded_hal::Error<GpioError>>`,
    `impl NumericScreen` (`Number = i16`, `DRAWABLE_RANGE = FOUR_DIGITS`), and
    `impl Draw`.
  - `pub async fn display_loop<S: Draw>(gauge: &dyn Gauge, screen: &mut S)`.
  - `main.rs` keeps `mod display;` private (section 4: `pub mod` makes
    `async_fn_in_trait` fire and fails the gate).
  - In `main.rs`: `const TASK_NAMES: [&str; 2] = ["moisture sensor", "display"];`
    (grows in Tasks 10 and 13).

**Concepts first:** interior mutability (`Cell`) and why `&MoistureGauge`
can be shared while `&mut MoistureSensor` can't; privacy per module, so only
the sensor calls `store`; implementing a foreign trait for a local type;
`Result` versus `Option`; `async fn` and `Timer::after`; boxed trait-object
futures; `join_array` versus `select_array` (section 5); a supertrait
(`Draw: NumericScreen`); `async fn` in a trait and why it isn't dyn
compatible; the orphan rule and the newtype around the driver; a
constructor that does the setup (`Tm1637Screen::init`) so an uninitialized
screen can't exist.

**Questions:** Why does `run` take `&mut self` but `display_loop` only
`&dyn Gauge`? What does `select_array` return? What keeps `TASK_NAMES[index]`
in bounds, and which part of that does the compiler not check?

- [ ] **Step 1: `Sample` and `MoistureGauge`.** `latest()` maps a sample to
  `Reading { value: Value::from(percent), ts }`.
- [ ] **Step 2: `sample()`.** Averages 30 `read()` calls in a `u32`, computes
  the percent with the calibration, and stamps `ts` from
  `SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()`. Until SNTP
  (Task 11), `ts` counts from 1970 at boot; that's expected for now.
- [ ] **Step 3: `run()`** with the change-only logging from section 4 (local
  `failing: bool`, `warn` with `{e:#}` on the first error, `info` on
  recovery, `debug` on every `Ok`) and a 500 ms refresh interval.
- [ ] **Step 4: `display.rs`.** `Draw`, `Tm1637Screen` (`init`, both impls),
  and `display_loop`. Every 500 ms it computes
  `readout_for::<S>(gauge.latest())` and calls `screen.show(readout).await`
  only when the readout differs from a local
  `last_drawn: Option<Readout<S::Number>>` (`None` at start and after a
  failed write, `Some(readout)` after a successful one). `show` draws per the
  table in section 4: `Number(n)` with `options().r_i16_4(n)` (correct for
  zero in 0.5.3), and each word as the full 4-character string
  (`"----"`, `"HIGH"`, `"  LO"`, `"  On"`, `" OFF"`) with
  `options().str(..)`, because `Align<4>` doesn't pad. Write failures use the
  same change-only logging.
- [ ] **Step 5: `main.rs`.** Build the display with
  `block_on(display::Tm1637Screen::init(tm1637))` (it replaces today's
  separate `init()` call), and pass `&mut screen` to `display_loop`. Build
  the 2-task array with explicit type `[Pin<Box<dyn Future<Output = ()> + '_>>; 2]`, then
  `let ((), index) = block_on(select_array(tasks));` and return an error
  naming `TASK_NAMES[index]`. Until Task 6, `main` returns that error.
- [ ] **Step 6: Board.** Percentage shows as before; dashes for the first
  500 ms at most.
- [ ] **Step 7: Prove `select_array`.** Temporarily make `display_loop` return
  after 10 iterations. Expected: an error naming the `display` task. Revert.
- [ ] **Step 8: Review Focus 4.** Unplug AOUT while running. Note what the
  display shows. Expected per today's design: a high percentage, not dashes.
- [ ] **Step 9:** Gate, review, commit:
  `feat: split moisture sensor and gauge, run tasks with select_array`.

---

### Task 6: `main`/`run`, `fail_and_restart`, `logging::init`, `.context()`

**Files:** Create `src/restart.rs`, `src/logging.rs`; modify `src/main.rs`.

**Interfaces (sections 5 and 6):**
- Produces: `pub fn restart::fail_and_restart(reason: &anyhow::Error) -> !`
  (logs `{reason:?}`, waits 10 000 ms with `FreeRtos::delay_ms`, calls
  `esp_idf_svc::hal::reset::restart()`); `pub fn logging::init() -> anyhow::Result<()>`
  with `DEBUG_TARGETS` (section 6; only modules that exist so far, plus
  `lector_humedad::logging` and `lector_humedad::restart`);
  `fn run() -> anyhow::Result<()>` called by a `main()` that returns nothing.

**Concepts first:** the never type `!`; why `main()` doesn't return
`Result` here; `anyhow::Context` and error chains; `{e}` versus `{e:#}` versus
`{e:?}`; ESP-IDF's exact-match log tags (section 6); log or propagate, not
both.

**Questions:** Without `.context`, what would the log say if
`AdcDriver::new` failed? Why would a module missing from `DEBUG_TARGETS` log
less, never more?

- [ ] **Step 1:** `restart.rs`.
- [ ] **Step 2:** `logging.rs`; delete the two `set_target_level` calls from
  `main.rs`.
- [ ] **Step 3:** Split `main` into `main` and `run`; add `.context(..)` to
  every existing setup step, with the messages from section 5.
- [ ] **Step 4: Prove the restart path.** Temporarily add
  `anyhow::bail!("test failure")` at the start of `run`. Expected: one
  `error` line with the chain, a 10 s pause, a reboot. Revert.
- [ ] **Step 5: Prove the log filter.** Temporarily remove
  `lector_humedad::moisture_sensor` from `DEBUG_TARGETS`. Expected: its
  `debug` lines disappear. Revert.
- [ ] **Step 6:** Gate, review, commit:
  `feat: add restart path, logging policy and error context`.

---

### Task 7: Build-time configuration (`toml-cfg`)

**Files:** Modify `Cargo.toml`, `build.rs`, `src/main.rs`,
`.github/workflows/rust_ci.yml`; create `cfg.toml.example`; create your own
`cfg.toml` (already gitignored).

**Interfaces (section 5, "Configuration"):** `Config` with `wifi_ssid`,
`wifi_password`, `mqtt_host`, `mqtt_password` (all `#[default("")]`) and
`sntp_server` (`#[default("0.pool.ntp.org")]`), declared in both `build.rs`
and `main.rs`. The TOML table name is the package name: `[lector-humedad]`.

**Concepts first:** build scripts run on the host, before compiling the
crate; how `toml-cfg` turns a file into a `const CONFIG`; why the struct is
declared twice; `panic!` in a build script fails the build.

- [ ] **Step 1:** Add `toml-cfg` as a dependency and a build dependency
  (check the version `std-training` uses; take the current one if it builds).
- [ ] **Step 2:** `cfg.toml.example` with the five keys and obvious
  placeholders (for example `"your-ssid"`).
- [ ] **Step 3:** `build.rs` fails if `cfg.toml` is missing, if any value
  equals its placeholder, or if any required value is empty. Keep
  `embuild::espidf::sysenv::output()`.
- [ ] **Step 4:** `main.rs` declares `Config`; log `CONFIG.mqtt_host` once at
  `info` to see it works (never the passwords). The other fields have no
  reader until Tasks 9 to 12. No attribute is needed for that: with
  `toml-cfg`, unread `Config` fields don't trigger `dead_code` (checked on a
  host mock with 0.1.3 and 0.2.0).
- [ ] **Step 5: Prove each failure** (Review Focus 5): build without
  `cfg.toml`, with the example copied unchanged, and with one value set to
  `""`. Expected: three build errors, each naming the problem. Also check
  that editing `cfg.toml` alone triggers a rebuild; if it doesn't, `build.rs`
  needs `println!("cargo:rerun-if-changed=cfg.toml")`.
- [ ] **Step 6: CI.** `cfg.toml` is gitignored, so a clean checkout has none
  and the gate's clippy and build would fail. In
  `.github/workflows/rust_ci.yml`, add a step before `Run checks` that writes
  a `cfg.toml` with made-up values (not the example's placeholders).
- [ ] **Step 7:** Gate, review, commit, push: `feat: add build-time configuration`.
  Expected on GitHub: the job is green.

---

### Task 8: `topics` and `payload` (test-first)

**Files:** Create `fak-core/src/topics.rs`, `fak-core/src/payload.rs`; modify
`fak-core/src/lib.rs`.

**Interfaces (section 3):**
- Produces: `pub struct DeviceId(String)` (derive `Debug, Clone, PartialEq, Eq`)
  with `from_mac(mac: [u8; 6]) -> Self`, `as_str(&self) -> &str`, and
  `Display`; `availability_topic(device_id: &DeviceId) -> String`;
  `reading_topic(device_id: &DeviceId, reading_id: &str) -> String`;
  `payload::OFFLINE: &str`; `payload::online(ts: u64) -> String`;
  `payload::reading(reading: &Reading) -> String`.

**Concepts first:** newtypes and private fields as a format guarantee;
`format!` with `{:02x}`; `core::fmt::Write` and `write!` into a `String`;
raw strings `r#"..."#` for JSON in tests.

- [ ] **Step 1: Failing tests** (MAC `[0xa0, 0xb7, 0x65, 0x3f, 0x1c, 0x22]`
  unless noted):

  | Test | Assertion |
  |---|---|
  | `device_id_is_lowercase_hex_without_separators` | `as_str() == "a0b7653f1c22"` |
  | `device_id_keeps_leading_zeros` | MAC `[0x0a, 0, 1, 2, 3, 4]` gives `"0a0001020304"` |
  | `device_id_displays_as_its_string` | `format!("{id}") == id.as_str()` |
  | `availability_topic_layout` | `"fak/v1/devices/a0b7653f1c22/meta/availability"` |
  | `reading_topic_layout` | with `"moisture-01"`: `"fak/v1/devices/a0b7653f1c22/readings/moisture-01"` |
  | `integer_reading_payload` | `Integer(42)`, ts `1700000000`: `{"value":42,"ts":1700000000}` |
  | `negative_integer_reading_payload` | `Integer(-5)`: `{"value":-5,"ts":1700000000}` |
  | `bool_reading_payload` | `Bool(true)`: `{"value":true,"ts":1700000000}` |
  | `online_payload` | `online(1700000000) == {"state":"online","ts":1700000000}` |
  | `offline_payload` | `OFFLINE == {"state":"offline"}` |

- [ ] **Step 2:** `cargo test-core`: fails to compile.
- [ ] **Step 3: Implement.** `payload::reading` matches on `Value` (no
  wildcard arm, so a new variant forces an update).
- [ ] **Step 4:** `cargo test-core`: passes.
- [ ] **Step 5:** Gate, review, commit: `feat(core): add device ID, topics and payloads`.

---

### Task 9: `wifi::connect` and the device ID

**Files:** Create `src/wifi.rs`; modify `src/main.rs`, `src/logging.rs`.

**Interfaces (section 2):** `pub fn connect(ssid: &str, password: &str, modem: Modem<'static>, sysloop: EspSystemEventLoop) -> anyhow::Result<EspWifi<'static>>`.
Consumes `DeviceId::from_mac` (Task 8).

**Concepts first:** `EspWifi` versus `BlockingWifi` (wrap for setup, keep the
inner `EspWifi`); NVS and why radio calibration needs it; the three fixes
from `std-training` commit `f2223b5`; one `unsafe` FFI call and its SAFETY
comment; `Peripherals::take` and moving `modem` out.

**Read first:** `/home/krapp/dev/fak/std-training/common/lib/wifi/src/lib.rs`
(adapt, don't copy: the `Peripheral` trait there doesn't exist in this HAL).

- [ ] **Step 1:** `connect` with NVS, `esp_wifi_set_max_tx_power(30)` after
  `start()`, and up to 5 attempts (`disconnect()` and 500 ms between them).
- [ ] **Step 2:** In `run()`: `Peripherals::take`, `EspSystemEventLoop::take`,
  `wifi::connect`, `get_mac(WifiDeviceId::Sta)`, `DeviceId::from_mac`, then
  `log::info!("device id: {device_id}")`. Each with its `.context`.
- [ ] **Step 3:** Add `lector_humedad::wifi` to `DEBUG_TARGETS`.
- [ ] **Step 4: Board.** Expected: WiFi connects, the device ID is logged,
  no "NVS has not been initialized" line, moisture still shown.
- [ ] **Step 5: Prove the failure path.** Wrong SSID in `cfg.toml`.
  Expected: 5 attempts, then `fail_and_restart` with "connecting to WiFi" in
  the chain. Restore.
- [ ] **Step 6:** Gate, review, commit: `feat: connect to WiFi at boot`.

---

### Task 10: `LinkMonitor` (test-first) and `wifi::monitor`

**Files:** Create `fak-core/src/link.rs`; modify `fak-core/src/lib.rs`,
`src/wifi.rs`, `src/main.rs`.

**Interfaces (section 2):**
- Produces: `pub const DOWN_LIMIT: core::time::Duration` (60 s);
  `#[must_use] pub enum Action { Wait, Reconnect, Retry, Recovered, Restart }`;
  `#[derive(Debug, Default)] pub struct LinkMonitor`;
  `pub fn step(&mut self, is_up: bool, now: core::time::Duration) -> Action`;
  `pub async fn wifi::monitor(wifi: &mut EspWifi<'static>)`.
- `TASK_NAMES` becomes `[&str; 3]`, adding `"WiFi monitor"`.

**Concepts first:** small state machines as `enum` plus `struct`; testing
time without waiting (passing `now` in); `saturating_sub`; `#[must_use]` on
a type; `Default` derive.

- [ ] **Step 1: Failing tests** (`s(n)` meaning `Duration::from_secs(n)`;
  each test uses a fresh `LinkMonitor::default()` and the calls listed in
  order):

  | Test | Calls and expected results |
  |---|---|
  | `up_without_a_drop_waits` | `step(true, s(0))` is `Wait` |
  | `first_down_check_reconnects` | `step(false, s(10))` is `Reconnect` |
  | `second_down_check_retries` | `step(false, s(10))`, then `step(false, s(15))` is `Retry` |
  | `down_just_under_the_limit_retries` | `step(false, s(10))`, then `step(false, s(10) + DOWN_LIMIT - Duration::from_millis(1))` is `Retry` |
  | `down_at_the_limit_restarts` | `step(false, s(10))`, then `step(false, s(10) + DOWN_LIMIT)` is `Restart` |
  | `recovery_is_reported_once` | `step(false, s(10))`, `step(true, s(20))` is `Recovered`, `step(true, s(25))` is `Wait` |
  | `drop_after_recovery_reconnects_and_restarts_the_timer` | down at 10, up at 20, `step(false, s(30))` is `Reconnect`, `step(false, s(30) + DOWN_LIMIT - Duration::from_millis(1))` is `Retry` |
  | `clock_going_backwards_does_not_panic` | `step(false, s(10))`, then `step(false, s(5))` is `Retry` |

- [ ] **Step 2:** `cargo test-core`: fails to compile.
- [ ] **Step 3: Implement `step`** from the table in section 2.
- [ ] **Step 4:** `cargo test-core`: passes.
- [ ] **Step 5: `wifi::monitor`.** Every 5 s: `is_up()` (an `Err` counts as
  down), `now` from `embassy_time::Instant`, `step`, then act (section 2).
  `Reconnect` and `Retry` both call `wifi.connect()`, unless
  `wifi.is_connected()` is `Ok(true)` (link up, still waiting for an IP
  address); a failing call is logged at `warn` for `Reconnect` and at
  `debug` for `Retry`.
- [ ] **Step 6:** Add it as the third task, with its name third in
  `TASK_NAMES`.
- [ ] **Step 7: Board** (Review Focus 3). Switch the access point off for
  about 20 s: one `warn`, one `info`, no restart. Then for over 60 s: a
  restart with "WiFi" in the reason. Display keeps updating throughout.
  Note what `wifi.connect()` returns on the retries (the `debug` lines), and
  whether the WiFi library prints its own line on each retry. Record both in
  the spec's section 2.
- [ ] **Step 8:** Gate, review, commit: `feat: monitor WiFi and restart after a long drop`.

---

### Task 11: `sntp::sync`

**Files:** Create `src/sntp.rs`; modify `src/main.rs`, `src/logging.rs`.

**Interfaces (section 5 notes):** `pub fn sync(server: &str, timeout: embassy_time::Duration) -> anyhow::Result<EspSntp<'static>>`.

**Concepts first:** why the returned value must be kept alive (`_sntp`
versus `_`); polling with a deadline; `SntpConf::servers` sized by a Kconfig
value, and why `[server]` stops compiling if that value changes.

- [ ] **Step 1:** Implement with `SntpConf { servers: [server], ..Default::default() }`,
  polling `get_sync_status()` every 100 ms with `FreeRtos::delay_ms` until
  `SyncStatus::Completed` or the timeout.
- [ ] **Step 2:** Call it after WiFi with 30 s, keep `_sntp`, log the synced
  time at `info`. Sensors must start after this line.
- [ ] **Step 3: Board.** Expected: the logged time is today's date in UTC.
- [ ] **Step 4: Prove the timeout.** Set `sntp_server` to an IP that doesn't
  answer. Expected: restart after ~30 s with "syncing time over SNTP". Restore.
- [ ] **Step 5:** Gate, review, commit: `feat: sync time over SNTP at boot`.

---

### Task 12: MQTT connection, callback, and `online`

**Files:** Create `src/mqtt.rs`; modify `sdkconfig.defaults`, `Cargo.toml`,
`src/main.rs`, `src/logging.rs`.

**Prerequisite:** the board's device ID (logged since Task 9) is in the
Mosquitto password file, per `fak-supporting`'s `README.md` (section 3,
"Manual broker setup").

**Interfaces (section 3):**
- Produces: `pub struct MqttHandle { requests_tx: SyncSender<PublishRequest> }`;
  private `enum PublishRequest { Online }` (the `Reading` variant arrives in
  Task 13, with the code that builds it);
  `pub fn start(device_id: &DeviceId, host: &str, password: &str) -> anyhow::Result<MqttHandle>`;
  `pub fn wait_connected(timeout: embassy_time::Duration) -> anyhow::Result<()>`;
  module-level `static CONNECTED: Signal<CriticalSectionRawMutex, ()>`;
  private `fn mqtt_thread(client, requests_rx, device_id: DeviceId) -> !`.
- Consumes: `DeviceId`, `availability_topic`, `payload::OFFLINE`,
  `payload::online` (Task 8).

**Concepts first:** threads, `move` closures, and `Send`; bounded channels
and `try_send` versus `send`; why the callback must never block or call the
client (section 3); `Signal` and `static`; `Box::leak` for `&'static` data;
FFI pointers and the SAFETY argument in `messageExpiry`. This is the
heaviest task for concurrency: Claude explains each piece before you write
it.

- [ ] **Step 1:** Add the `espressif/mqtt` 1.1.0 remote component to
  `Cargo.toml` (section 5, "Build configuration"; ESP-IDF v6 no longer ships
  esp-mqtt). `CONFIG_MQTT_PROTOCOL_5=y` in `sdkconfig.defaults`; add
  `embassy-sync` (locked at 0.8.0). Build once: confirms open item 1 (the
  `esp_mqtt5_client_set_publish_property` binding exists) and that the
  component manager resolved exactly 1.1.0.
- [ ] **Step 2:** The client configuration table from section 3.
- [ ] **Step 3:** The event callback (three things only, section 3).
- [ ] **Step 4:** `start`: channel, `new_cb`, thread with
  `stack_size(6144)`. Nothing reads `MqttHandle::requests_tx` until
  `send_reading` exists, so put
  `#[expect(dead_code, reason = "send_reading uses it in Task 13")]` on the
  field.
- [ ] **Step 5:** `mqtt_thread` handling `Online`, the only request so far:
  `online_props` leaked once, setter before every publish (with its SAFETY
  comment), availability topic built once.
- [ ] **Step 6:** `wait_connected`, with the fixed-message `map_err`.
- [ ] **Step 7:** In `run()`, after SNTP: `mqtt::start` and
  `wait_connected(30 s)`, each with `.context`. Bind the handle as `_mqtt`
  until Task 13 uses it (a plain `mqtt` is an unused variable, and a bare
  `_` would drop the handle at once).
- [ ] **Step 8: Board.** With `mosquitto_sub -v -t 'fak/v1/devices/#'` (and
  the broker credentials) on your machine: expected a retained
  `{"state":"online","ts":...}`. Power the board off: after about
  45 s (keep-alive) plus 60 s (will delay), `{"state":"offline"}`. Also
  check: the `online` came from a `try_send` on esp-mqtt's task (open item 4).
  Log the MQTT thread's stack high-water mark once (open item 2).
- [ ] **Step 9: Review Focus 1.** Wrong `mqtt_password`. Expected: esp-mqtt
  logs the refusal, then restart after ~30 s with "waiting for the MQTT
  broker". Restore.
- [ ] **Step 10:** Gate, review, commit: `feat: connect to MQTT and publish availability`.

---

### Task 13: Publishing readings

**Files:** Modify `src/mqtt.rs`, `src/main.rs`.

**Interfaces (section 3):**
- Produces: `fn MqttHandle::send_reading(&self, reading_id: &'static str, payload: String)`
  (drop and `warn` when full); `pub async fn publish_loop(gauges: &[&dyn Gauge], mqtt: &MqttHandle)`;
  `reading_props` with `message_expiry_interval: 180`.
- `PublishRequest` gains `Reading { reading_id: &'static str, payload: String }`.
- `TASK_NAMES` becomes `[&str; 4]`, with `"MQTT publish"` inserted third,
  before `"WiFi monitor"`, so the order matches section 5.

**Concepts first:** slices of trait objects (`&[&dyn Gauge]`); `Ticker`
versus a new `Timer` each loop; why the first reading comes 60 s after start
(section 3).

- [ ] **Step 1:** The `Reading` variant and `send_reading`. Remove the
  `#[expect(dead_code)]` from `requests_tx` and rename `_mqtt` to `mqtt`.
- [ ] **Step 2:** The `Reading` arm of `mqtt_thread`: set `reading_props`
  (SAFETY comment), publish QoS 1, retained, to `reading_topic(..)`. Setter
  error: log at `error` and skip. Publish error: log at `warn`.
- [ ] **Step 3:** `publish_loop`: one `Ticker::every(60 s)`; skip gauges with
  `None`; `payload::reading(&reading)`.
- [ ] **Step 4:** `let gauges: [&dyn Gauge; 1] = [&moisture_gauge];` and the
  publish task, inserted third in the task array, the same position as its
  name in `TASK_NAMES`. The boot sequence now matches section 5 except for
  the error codes (display first, `setup_network`), which arrive in Task 14.
- [ ] **Step 5: Board.** Expected: about 60 s after boot, a retained message
  on `.../readings/moisture-01` like `{"value":43,"ts":...}`, then one per
  minute.
- [ ] **Step 6: Expiry.** Power the board off. Expected: the retained reading
  disappears from the broker within 180 s of its last publish.
- [ ] **Step 7: Review Focus 2.** Stop the broker for 3 minutes, then start
  it. Expected: `warn` lines but no restart; display keeps updating;
  `online` again on reconnect; readings resume. Note the longest gap in the
  sensor's debug lines (open item 3: how long the MQTT thread waited on the
  API lock, which must not stall the other tasks).
- [ ] **Step 8:** Update section 7 of the spec with what Tasks 12 and 13
  measured (items 1 to 4), then gate, review, commit:
  `feat: publish moisture readings over MQTT`.

---

### Task 14: Error codes on the display for setup failures

**Files:** Create `fak-core/src/fault.rs`; modify `fak-core/src/lib.rs`,
`fak-core/src/readout.rs`, `src/display.rs`, `src/main.rs`.

**Interfaces (section 4, `fault.rs`, and section 5, "Boot sequence"):**
- Produces in `fak-core`:
  `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Fault { Wifi, Sntp, Mqtt }`,
  `#[must_use] pub const fn Fault::code(self) -> u8` (1, 2, 3),
  `impl fmt::Display for Fault` (`E-01`, `E-02`, `E-03`), and a new variant
  `Readout::Fault(Fault)`.
- Produces in `main.rs`: `struct Network { wifi, mqtt, _sntp }`,
  `struct SetupError { fault: Fault, source: anyhow::Error }`,
  `fn setup_network(modem, sysloop) -> Result<Network, SetupError>`,
  `fn connect_wifi(..) -> anyhow::Result<(EspWifi<'static>, DeviceId)>`,
  `fn connect_mqtt(&DeviceId) -> anyhow::Result<MqttHandle>`, and
  `fn show_or_warn(screen: &mut display::Tm1637Screen, readout: Readout<i16>)`.
- Consumes: everything `run()` set up in Tasks 6, 9, 11, 12, and 13.

**Concepts first:** an enum as the list of valid codes; `match` without a
wildcard arm; implementing `Display` by hand with `write!` and `{:02}`;
`to_string()` comes free with `Display`; adding a variant and letting the
compiler list every `match` to fix (E0004); `map_err` with a closure that
builds a struct, and field shorthand; destructuring a struct in a `match`
pattern; `block_on` outside the task array; borrowing two fields of one
struct separately; why a failed display write is logged and not propagated.

**Questions:** Why does `SetupError` not need to implement
`std::error::Error`? What would the display show for a fault with code 100,
and what stops that today? Why does the failure arm in `run()` call
`fail_and_restart` and not `return Err(..)`, and what would happen to the
display's pins if it returned?

- [ ] **Step 1: Failing tests** in `fak-core/src/fault.rs`:

  | Test | Assertion |
  |---|---|
  | `wifi_is_code_one` | `Fault::Wifi.code() == 1` |
  | `sntp_is_code_two` | `Fault::Sntp.code() == 2` |
  | `mqtt_is_code_three` | `Fault::Mqtt.code() == 3` |
  | `wifi_displays_as_e_01` | `Fault::Wifi.to_string() == "E-01"` |
  | `sntp_displays_as_e_02` | `Fault::Sntp.to_string() == "E-02"` |
  | `mqtt_displays_as_e_03` | `Fault::Mqtt.to_string() == "E-03"` |

- [ ] **Step 2:** `cargo test-core`: fails to compile.
- [ ] **Step 3: Implement** `Fault`, `code`, and `Display`; register the
  module in `lib.rs`. `cargo test-core`: passes.
- [ ] **Step 4: `Readout::Fault(Fault)`.** Add the variant and update the
  doc comment on `Readout` (section 4). `cargo test-core` still passes:
  `readout_for` matches on `Value`, so it doesn't change. Then
  `cargo build`. Expected: E0004 in `Tm1637Screen::show`, naming the missing
  `Fault(_)` pattern.
- [ ] **Step 5: Draw it.** The new arm of `show` draws
  `options().str(&fault.to_string())`. `cargo build` passes.
- [ ] **Step 6: Restructure `run()`** to the boot sequence in section 5:
  display first, then `show_or_warn(&mut screen, Readout::NoReading)`, then
  `setup_network` with its `match` (the failure arm shows the code and calls
  `fail_and_restart` itself, so `screen` isn't dropped), then the sensor,
  then the tasks. Move
  the WiFi, SNTP, and MQTT steps into `connect_wifi`, `setup_network`, and
  `connect_mqtt`, keeping every `.context(..)` message.
- [ ] **Step 7: Board, normal boot.** Expected: `----` while the network
  comes up, then the percentage. Readings still arrive on the broker.
- [ ] **Step 8: Board, each code.** One at a time, restoring `cfg.toml`
  after each:
  - Wrong `wifi_ssid`: `E-01`, steady and readable for the whole 10 s, then
    a restart.
  - `sntp_server` set to an IP that doesn't answer: `E-02`.
  - Wrong `mqtt_password`: `E-03`.

  In each case the log's `error` chain starts with
  `setup failed with code E-0x`, and the display alternates between `----`
  and the code across restarts.
- [ ] **Step 9:** Gate, review, commit:
  `feat: show an error code on the display when setup fails`.
