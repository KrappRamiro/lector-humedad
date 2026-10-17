# Workspace Split Implementation Plan

> **How this plan is executed:** the project owner writes all the code.
> Claude acts as a mentor: before each task it explains the task's concepts,
> answers questions, and after each task it reviews the owner's diff. Claude
> does not write the code. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the project into a Cargo workspace with a host-tested
`fak-core` crate, so the existing tests run for the first time, locally and
in CI.

**Architecture:** The root `Cargo.toml` stays the firmware package and gains
a `[workspace]` with one member, `fak-core`. `fak-core` holds pure logic with
no ESP-IDF dependency and is tested on the development machine through a
cargo alias.

**Tech Stack:** Cargo workspaces, nightly Rust with `build-std`, libtest,
GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-17-workspace-split-design.md`

## Global Constraints

- `fak-core` has no `esp-idf-*` dependency and no `log` dependency.
- `fak-core/src/lib.rs` starts with `#![forbid(unsafe_code)]`.
- `edition = "2024"` and `rust-version = "1.99"` live in `[workspace.package]`
  only; both crates use `<field>.workspace = true`.
- The firmware binary keeps `harness = false`.
- Gate after every task: `cargo fmt --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` pass.
- Advice after every task: run
  `cargo clippy --workspace --all-targets -- -W clippy::pedantic`, read every
  finding, fix the ones in code the task is meant to change.
- Then `/code-review` on the diff, then commit.
- Never run plain `cargo test`: it flashes the board. Use `cargo test-core`.

## Review Focus

Conditions no test in this plan exercises, most likely first:

1. **`cargo test` typed out of habit.** It builds for RISC-V and flashes the
   board. Expected: nothing breaks, but no tests run. Covered by the alias
   and by the Task 1 lesson; there's nothing to test.
2. **`cargo build` from inside `fak-core/`.** Cargo finds the workspace root
   and its `.cargo/config.toml`, so it builds `fak-core` for RISC-V. Expected:
   it compiles. Task 1, Step 4 checks it once.
3. **A stale `Cargo.lock` in CI.** Expected: CI fails with `--locked` instead
   of resolving new versions. Task 3, Step 3 checks it once by hand.
4. **The constants and the tests drift apart.** The firmware's
   `DRY_READING`/`WET_READING` and the test module's copies are separate
   values after Task 2. Expected: tests still describe the real sensor.
   Task 2 keeps them equal; the MQTT plan replaces both with `Calibration`.
5. **First host build is slow** (about 20 s, `build-std` builds the host's
   `std`). Expected: later runs are fast. Nothing to test.

---

### Task 1: Workspace, empty `fak-core`, alias, committed lock file

**Files:**
- Modify: `Cargo.toml`
- Create: `fak-core/Cargo.toml`, `fak-core/src/lib.rs`
- Modify: `.cargo/config.toml` (add `[alias]`)
- Modify: `.gitignore` (remove the `Cargo.lock` line)

**Interfaces:**
- Produces: a workspace member named `fak-core`, imported as `fak_core`; the
  command `cargo test-core`.

**Concepts first (Claude explains):**
- What a workspace is: one `Cargo.lock` and one `target/` shared by several
  crates. Which manifest sections only work at the root (`[patch]`,
  `[profile]`).
- `[workspace.package]` and `<field>.workspace = true` (inheritance).
- Why your 5 tests never ran: `harness = false`, `--test`, and why
  `#[allow(unused_imports)]` in `moisture_sensor.rs` was the clue
  (spec section 1).
- How `.cargo/config.toml` is found (searched from the current directory
  upwards) and how `--target` overrides `[build] target`.
- Crate attributes: `#![...]` versus `#[...]`, and `forbid` versus `deny`.

**Read first:** spec sections 1 to 3; the Cargo book pages "Workspaces" and
"Configuration" (`[alias]`).

**Questions to answer before writing:**
- Why does the root stay a package instead of becoming a virtual workspace?
- If `fak-core` had `harness = false` too, what would `cargo test-core` do?

- [ ] **Step 1: Edit the root `Cargo.toml`.** Add `[workspace]` with
  `members = ["fak-core"]` and `[workspace.package]` with
  `edition = "2024"` and `rust-version = "1.99"`. In `[package]`, replace
  `edition` and `rust-version` with `edition.workspace = true` and
  `rust-version.workspace = true`. Don't add the `fak-core` dependency yet
  (nothing uses it, and an unused dependency is noise in the diff).
- [ ] **Step 2: Create `fak-core`.** `fak-core/Cargo.toml` with
  `name = "fak-core"`, `version = "0.1.0"`, and the two inherited fields.
  `fak-core/src/lib.rs` contains only `#![forbid(unsafe_code)]`.
- [ ] **Step 3: Add the alias** to `.cargo/config.toml`:
  `test-core = "test -p fak-core --target x86_64-unknown-linux-gnu"`.
- [ ] **Step 4: Verify.**
  - `cargo test-core`: expected `running 0 tests` and `test result: ok`
    (twice: unit tests and doc-tests).
  - `cargo build`: the firmware builds as before.
  - `cd fak-core && cargo build`: builds for RISC-V (Review Focus 2).
  - `cargo run`: the board still shows a moisture percentage.
- [ ] **Step 5: Commit `Cargo.lock`.** Remove `Cargo.lock` from
  `.gitignore`, then stage it with the other files.
- [ ] **Step 6: Gate, advice, review, commit.** Message:
  `build: add fak-core workspace member and test-core alias`.

---

### Task 2: Move `percent_moisture_from_reading` and its tests

**Files:**
- Create: `fak-core/src/moisture.rs`
- Modify: `fak-core/src/lib.rs` (declare `pub mod moisture;`)
- Modify: `src/moisture_sensor.rs` (remove the function and the tests; call
  the moved function; move the `log::debug!` into `read()`)
- Modify: `Cargo.toml` (add `fak-core = { path = "fak-core" }`)

**Interfaces:**
- Consumes: the `fak-core` member from Task 1.
- Produces:
  `pub fn fak_core::moisture::percent_moisture_from_reading(raw_reading: u16, dry_reading: u16, wet_reading: u16) -> u16`,
  unchanged except for `pub`.

**Concepts first (Claude explains):**
- Modules versus crates; `pub mod`; how `use fak_core::moisture::...` finds
  the item; why `-` becomes `_`.
- Privacy is per module: what `pub`, `pub(crate)`, and private mean across
  a crate boundary.
- Unit tests in `#[cfg(test)] mod tests` next to the code, and `use super::*`.
- Why a pure function shouldn't log, and where the log line goes instead.
- A "pure move" commit: why reviewers like "move" and "change" apart.

**Read first:** workspace spec section 4 ("What moves in this change").

**Questions to answer before writing:**
- After the move, which file decides the calibration values, and which file
  only does arithmetic?
- The test module needs `DRY_READING` and `WET_READING`. Why do they get
  their own copies instead of importing the firmware's?

- [ ] **Step 1: Run the tests before moving anything.** `cargo test-core`
  still shows 0 tests. This is the "before" picture.
- [ ] **Step 2: Create `fak-core/src/moisture.rs`** with the function body
  copied unchanged, made `pub`, without its `log::debug!` line.
- [ ] **Step 3: Move the 5 tests** into a `#[cfg(test)] mod tests` in the
  same file, with test-local `const DRY_READING: u16 = 2309;` and
  `const WET_READING: u16 = 780;`. Test names and assertions stay exactly as
  they are:

  | Test | Input | Expected |
  |---|---|---|
  | `raw_reading_at_dry_bound_is_zero_percent` | 2309 | 0 |
  | `raw_reading_at_wet_bound_is_hundred_percent` | 780 | 100 |
  | `raw_reading_above_dry_bound_clamps_to_zero_percent` | 2809 | 0 |
  | `raw_reading_below_wet_bound_clamps_to_hundred_percent` | 280 | 100 |
  | `raw_reading_at_midpoint_is_fifty_percent` | 1544 | 50 |

- [ ] **Step 4: Run them.** `cargo test-core`: expected `5 passed`. This is
  the first time these tests have ever run.
- [ ] **Step 5: Break one on purpose.** Change one expected value, run
  `cargo test-core`, read the failure output, then revert. (You learn what a
  failure looks like before you need to.)
- [ ] **Step 6: Update the firmware.** Add the `fak-core` dependency.
  `MoistureSensor::read()` calls `fak_core::moisture::percent_moisture_from_reading`
  and logs averaged millivolts and percent at `debug`. Delete the old
  function and the old test module from `src/moisture_sensor.rs`.
- [ ] **Step 7: Verify on the board.** `cargo run`: same percentage as
  before for the same soil, and the debug line now comes from
  `lector_humedad::moisture_sensor`.
- [ ] **Step 8: Gate, advice, review, commit.** Pedantic will flag the `as`
  casts; they're expected (spec section 7) and leave with `Calibration`.
  Message: `refactor: move moisture percent math to fak-core`.

---

### Task 3: CI runs the tests

**Files:**
- Modify: `.github/workflows/rust_ci.yml`

**Interfaces:**
- Consumes: the `test-core` alias and the committed `Cargo.lock`.

**Concepts first (Claude explains):**
- How the existing workflow's matrix turns into `cargo <command> <args>`.
- What `--locked` does, and why it only makes sense once `Cargo.lock` is
  committed.

**Read first:** workspace spec sections 5 and 6; the current
`rust_ci.yml`.

- [ ] **Step 1: Add the matrix entry** `command: test-core`,
  `args: --locked`.
- [ ] **Step 2: Add `--locked`** to the `build` args and to the `clippy`
  args, before clippy's `--`.
- [ ] **Step 3: Check `--locked` locally.** Run `cargo test-core --locked`:
  passes. Then temporarily pin a version the lock file doesn't have, for
  example `log = "=0.4.33"` (the lock has 0.4.34), and run it again: expected
  an error saying the lock file needs to be updated. Revert. (A requirement
  like `"0.4.30"` wouldn't do it: 0.4.34 already satisfies it, so the lock
  file stays valid.)
- [ ] **Step 4: Gate, review, commit, push.** Message:
  `ci: run fak-core tests and build with --locked`. Expected on GitHub: the
  `test-core` job is green and shows `5 passed`.
