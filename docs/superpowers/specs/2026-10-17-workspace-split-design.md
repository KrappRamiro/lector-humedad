# Workspace split design

Status: approved by the project owner on 2026-10-17, after two independent
reviews. Implementation plan:
`docs/superpowers/plans/2026-10-17-workspace-split.md`. Implement it before the
[MQTT integration](2026-10-15-mqtt-integration-design.md), which assumes this
layout.

This document describes how `lector-humedad` becomes a Cargo workspace with a
second crate, `fak-core`, that holds pure logic and runs its tests on the
development machine.

## 1. The problem

The project's unit tests have never run:

- The firmware binary sets `harness = false` (`Cargo.toml`). Cargo then
  doesn't pass `--test` to the compiler, and without `--test` the compiler
  removes every `#[test]` function. `cfg(test)` is still set, so the
  `mod tests` block compiles, but it's empty. That's why
  `src/moisture_sensor.rs` needs `#[allow(unused_imports)]` on
  `use super::*`: nothing is left to use it.
- `.cargo/config.toml` sets the build target to `riscv32imc-esp-espidf` and the
  runner to `espflash flash --monitor`. So `cargo test` builds a RISC-V binary,
  flashes it, and runs the normal `main()`.
- CI (`.github/workflows/rust_ci.yml`) runs `build`, `fmt`, and `clippy`, but
  no tests.

Tests that need the board can't run on the development machine anyway.
Logic that doesn't need the board can, once it lives in a crate without
ESP-IDF dependencies.

## 2. Layout

```
lector-humedad/
├── Cargo.toml            firmware package, plus [workspace]
├── Cargo.lock            one lock file for both crates (committed)
├── .cargo/config.toml    unchanged, plus an [alias]
├── build.rs              unchanged
├── sdkconfig.defaults    unchanged
├── rust-toolchain.toml   unchanged
├── src/                  firmware: anything that touches hardware or ESP-IDF
└── fak-core/
    ├── Cargo.toml        no esp-idf-* dependencies
    └── src/
        ├── lib.rs        declares the modules
        └── moisture.rs   moved from src/moisture_sensor.rs (section 4)
```

### Root `Cargo.toml`

Add:

```toml
[workspace]
members = ["fak-core"]

[workspace.package]
edition = "2024"
rust-version = "1.99"
```

In `[package]`, replace the `edition` and `rust-version` lines with:

```toml
edition.workspace = true
rust-version.workspace = true
```

and, under `[dependencies]`:

```toml
fak-core = { path = "fak-core" }
```

`[workspace.package]` holds the values both crates share, so they're written
once. Each crate opts in with `<field>.workspace = true`.

Everything else stays. `[patch.crates-io]` and `[profile.*]` only take effect
in the workspace root, which is where they already are.

### `fak-core/Cargo.toml`

```toml
[package]
name = "fak-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
```

No dependencies for now. It uses the default test harness, because running
`#[test]` functions is its purpose.

### `fak-core/src/lib.rs`

Starts with `#![forbid(unsafe_code)]`. The `!` makes it apply to the whole
crate, and `forbid` (unlike `deny`) can't be switched off further down. Pure
logic never needs `unsafe`, so this turns part of the boundary rule
(section 4) into a compiler check.

### Why the root stays a package

esp-idf-sys reads its settings, including `sdkconfig.defaults`, from the
*workspace directory*: the directory with `Cargo.lock` and `target/`
(`esp-idf-sys` `BUILD-OPTIONS.md`, locked commit `a6357d4`). With the firmware
package at the root, that directory doesn't change, so nothing moves.

Rejected alternatives:

- **A virtual workspace** (a root `Cargo.toml` with only `[workspace]`). The
  workspace directory would still be the root, so `sdkconfig.defaults` could
  stay. But the firmware's `Cargo.toml`, `src/`, and `build.rs` would move
  into a subdirectory, for no gain.
- **Two separate projects**, like `std-training`'s `common/lib/mqtt-messages`.
  That gives two lock files and two `target/` directories that can resolve
  different dependency versions.

### Naming

- `fak-core` is imported as `fak_core`, because Cargo turns `-` into `_`.
- The name `core` alone would collide with Rust's built-in `core` library.

## 3. Running things

| Command | What it does |
|---|---|
| `cargo test-core` | Runs `fak-core`'s tests on the development machine. |
| `cargo build`, `cargo run` | Builds and flashes the firmware, as today. |
| `cargo clippy --workspace --all-targets` | Checks both crates for the RISC-V target. |

Add the alias to `.cargo/config.toml`:

```toml
[alias]
test-core = "test -p fak-core --target x86_64-unknown-linux-gnu"
```

- `--target` overrides the `[build] target` default. The `espflash` runner
  applies only to `cfg(target_os = "espidf")`, so it doesn't apply here.
- `[unstable] build-std` applies to every target, so the first run builds the
  host's `std` from source (about 20 seconds). Later runs reuse it.
- Plain `cargo test` still flashes the board. Use the alias.
- The alias names the host target, `x86_64-unknown-linux-gnu`, which matches
  this machine and CI's `ubuntu-latest`. On another kind of machine (for
  example, an ARM Mac), change it to that machine's target.

Verified on a scratch workspace with this project's `.cargo/config.toml` and
`rust-toolchain.toml`: the alias builds and runs a `fak-core` test on the
host.

## 4. The boundary rule

> If it needs a peripheral or an ESP-IDF call, it stays in `src/`. Otherwise
> it goes in `fak-core`.

The build enforces this: `fak-core` has no ESP-IDF dependency, so
`use esp_idf_svc::...` there doesn't compile.

Further rules:

- **Move decisions, not stateful structs.** A struct whose job is holding
  hardware or enforcing who may write stays next to the code it protects.
  `MoistureGauge` stays in `src/moisture_sensor.rs`, because its private
  `store()` is what guarantees that only `MoistureSensor` writes it. Rust
  privacy is per module, and `pub(crate)` in `fak-core` would mean "only
  `fak-core`", so moving the gauge would force `store()` to be fully public.
- **`fak-core` doesn't log.** It returns values and errors, and the firmware
  decides what to log. So `fak-core` has no `log` dependency, and a pure
  function never logs on every call.

### What moves in this change

Only `percent_moisture_from_reading` and its five tests, into
`fak-core/src/moisture.rs`, as a pure move:

- The function becomes `pub`.
- Its `log::debug!` line moves to the caller, `MoistureSensor::read()`, which
  logs the same values (averaged millivolts and percent).
- `DRY_READING` and `WET_READING` stay in the firmware, because they describe
  one physical sensor unit. The test module in `fak-core` declares its own
  copies, with the same values.

The `Calibration` redesign is a separate change, from the MQTT spec. Keeping
"move code" and "change code" in separate commits keeps each one easy to
review.

### What moves later

The MQTT spec places these in `fak-core` when they're written:

| Module | Contents |
|---|---|
| `fak_core::moisture` | `Calibration` (with its `percent` method), `CalibrationError` |
| `fak_core::gauge` | `Value`, `Reading`, the `Gauge` trait |
| `fak_core::topics` | `DeviceId`, `availability_topic`, `reading_topic` |
| `fak_core::payload` | JSON formatting for readings and availability |
| `fak_core::readout` | Which number the display shows for a reading |
| `fak_core::link` | The WiFi monitor's decision: wait, reconnect, recovered, or restart |

The firmware implements `fak_core::gauge::Gauge` for its own `MoistureGauge`.
Rust allows that because the type is local to the firmware crate.

## 5. CI

Add a matrix entry to `.github/workflows/rust_ci.yml`:

```yaml
- command: test-core
  args: --locked
```

The workflow runs `cargo ${{ matrix.action.command }} ${{ matrix.action.args }}`,
so the alias works there unchanged.

Also add `--locked` to the existing `build` and `clippy` entries (before
their `--` separator, for `clippy`). With `--locked`, Cargo fails if
`Cargo.lock` doesn't match `Cargo.toml` instead of quietly resolving new
versions, so CI builds exactly what was committed (section 6).

## 6. `Cargo.lock`

Commit `Cargo.lock`, and remove it from `.gitignore`. For a binary, that's the
usual practice: it records the exact versions that were built and tested.
(This item moves here from the MQTT spec, section 5.)

## 7. Steps

Each step builds, passes its checks, and runs on the board before the next
one starts.

1. **Workspace and empty crate.** Add `[workspace]`, `fak-core` with a
   `lib.rs` holding only `#![forbid(unsafe_code)]`, the alias, and commit `Cargo.lock`. Check: `cargo test-core`
   runs with 0 tests, and the firmware still builds and shows a percentage.
2. **Move `percent_moisture_from_reading` and its tests.** Check:
   `cargo test-core` runs 5 tests, all passing, and the firmware still shows
   the same percentage.
3. **CI.** Add the `test-core` matrix entry. Check: the job runs and passes
   on GitHub.

After each step:

1. `cargo fmt`.
2. `cargo clippy --workspace --all-targets -- -D warnings`. This is the gate:
   it must pass.
3. `cargo clippy --workspace --all-targets -- -W clippy::pedantic`. This is
   advice, not a gate: read every finding as a lesson, fix the ones in code
   the step is meant to change, and leave the rest.
4. `/code-review`.

Why pedantic isn't a gate: step 2 moves code without changing it, and that
code already fails pedantic because of its `as` casts. The `Calibration`
change rewrites the same code and removes them. Making pedantic a gate would
force step 2 to change code too.

New code in `fak-core` should satisfy pedantic from the start. Two lints
apply only to public items in a library, so they're new to this project:

- `must_use_candidate`: mark functions with `#[must_use]` when ignoring their
  result is almost certainly a bug, such as `percent`.
- `missing_errors_doc`: a public function that returns `Result` documents when
  it fails, under an `# Errors` heading in its doc comment.
- `missing_panics_doc`: a public function that can panic, including through
  `expect`, documents when, under a `# Panics` heading. If it never panics in
  practice, the section says why.

Doc examples are tests too. `cargo test-core` also runs the code inside
`# Examples` sections of `fak-core`'s doc comments ("Doc-tests" in its
output). An example on `Calibration::percent` or `DeviceId::from_mac` is
documentation that CI checks.

## 8. Deferred

- **Property-based tests** (`proptest`) for `percent`: "the result is never
  above 100" and "more millivolts never gives more moisture", for every input.
  First check that `proptest` builds for the RISC-V target, because CI's
  `clippy --all-targets` compiles dev-dependencies for it.
- **Lint policy in `Cargo.toml`.** A `[workspace.lints]` section could hold
  the lint settings, so they don't depend on remembering a command line. Keep
  pedantic out of it until the `Calibration` change lands, for the reason in
  section 7. It
  could also forbid bare `unwrap()` with clippy's `unwrap_used` lint, instead
  of relying on the written rule in the MQTT spec's section 6. Tests would
  then need `allow-unwrap-in-tests = true` in `clippy.toml` (not yet checked
  on this toolchain).
- **On-board tests.** No mature test harness for ESP-IDF `std` targets is
  known to this project. The MQTT spec's open verification items serve as a
  manual checklist.
