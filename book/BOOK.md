---
title: Moisture over MQTT
language: en
type: code
url: https://claude.ai/artifact/F8pxUqfFzikVXSsoKgUXUL
---

Subject: taking the lector-humedad station (ESP32-C3 Super Mini, capacitive soil moisture sensor, TM1637 display, Rust on ESP-IDF `std`) from a single read-and-draw loop to a board that joins WiFi, syncs its clock over SNTP, and publishes its reading to the `fak` MQTT broker. The book follows `docs/superpowers/plans/2026-10-17-mqtt-integration.md` from Task 3 on, and its design, `docs/superpowers/specs/2026-10-15-mqtt-integration-design.md`. Every chapter maps to plan tasks, its exercise is the task itself, and the reader writes the code. When a chapter and the spec or plan disagree, the spec and plan are followed and the chapter is corrected.

Audience and reader level: the project owner. Has written Rust before but is rusty, and is relearning embedded and ESP32-specific concepts. Concurrency and threading are new and get explained from scratch before they are used. Before the book started, the reader had already built (outside the book): ADC oneshot reading of the sensor, the TM1637 display, the `fak-core` workspace split with `cargo test-core`, and the validated `Calibration` (plan Task 2, test-first).

Goal: after the last chapter the reader has finished the MQTT plan (Tasks 3 to 13) and understands each piece: the value types in `fak-core`, concurrency with `select_array`, build-time configuration, WiFi, SNTP, and MQTT.

## Chapters

1. Introduction: published
2. Values and gauges (plan Task 3): draft
3. The readout rule (plan Task 4): draft
4. Sharing a reading (plan Task 5, Steps 1 to 3): draft
5. Loops side by side (plan Task 5, Steps 4 to 9): draft
6. Failing well (plan Task 6): draft
7. Configuration at build time (plan Task 7): draft
8. Names on the wire (plan Task 8): draft
9. Joining WiFi (plan Task 9): draft
10. Watching the link (plan Task 10): draft
11. Setting the clock (plan Task 11): draft
12. Threads and channels (plan Task 12, concepts): draft
13. Talking to the broker (plan Task 12, implementation): draft
14. Publishing readings (plan Task 13): draft

## Facts already taught

- Chapter 1: the two crates (`fak-core` for pure logic tested on the host with `cargo test-core`, the firmware crate for everything that touches ESP-IDF) and why the split exists; the rule "move decisions, not stateful structs"; the target architecture (four boxed futures under one `block_on(select_array(..))`, plus a dedicated MQTT thread fed by `sync_channel(8)`); the locked versions and that ESP-IDF v6 no longer bundles esp-mqtt; the working loop of a chapter (failing test, implement, gate with `devenv test`, `/code-review`, commit, board check).

## Decisions

- 2026-11-04: the book is written in English and follows the MQTT plan and spec; chapters are produced on demand with `/book-chapter`, not all at once.
- 2026-11-04: plan Task 5 is split across chapters 4 and 5, and plan Task 12 across chapters 12 and 13, because they carry the most new concurrency material.
- 2026-11-04: the earlier standalone artifact "Humedad en Rust" predates the interactive-book plugin and is not reused; this book publishes to its own URL.

## Hechos

- 2026-10-17 to 2026-10-20: the workspace split happened because the project's unit tests had never run: `harness = false` on the binary strips every `#[test]`, and the espflash runner turns `cargo test` into flashing the board (workspace split spec, section 1; commits c189cb3, 715c3d1).
- 2026-10-20: the percentage math moved to `fak-core` as a loose function, `percent_moisture_from_reading(raw_reading, dry_reading, wet_reading)`, three `u16` with nothing stopping inverted limits (commit 715c3d1).
- 2026-10-20: an exported `LD_LIBRARY_PATH` in `devenv.nix` made nixd crash in the editor (`GLIBCXX_3.4.35 not found`); it was removed, and `LIBCLANG_PATH` plus nix-ld were enough (commit 5634f9f; project memory `project_no_ld_library_path.md`).
- 2026-10-20: `rust-toolchain.toml` was deleted after an unpinned nightly (2026-10-19) broke `std` for espidf in CI; devenv's lock now pins the toolchain (commit d0db4a1; project memory `project_workspace_split.md`).
- 2026-10-20: migration to ESP-IDF v6.1 (commit 672f517). Picolibc needs `--cfg espidf_picolibc` in rustflags, and a missing cfg fails silently. Changing `ESP_IDF_VERSION` needed `cargo clean -p esp-idf-sys` (project memory `project_esp_idf_v6.md`).
- 2026-10-20: the separate esp-idf-sys/hal/svc repositories were archived into the `esp-rs/esp-idf` monorepo; the project switched to crates.io releases (commit 7df7a8e).
- Before 2026-10-17: `embassy_executor` was tried and rejected because its task functions can't be generic, so each GPIO pin's ADC channel type needed a near-duplicate task; the project moved to `block_on` with generic async fns, and the MQTT spec later chose `select_array` over `join_array` (project memory `project_concurrency_architecture.md`).
- 2026-11-04: `Calibration` replaced the loose function, with a `const fn new` that rejects inverted limits; plan Task 2 Step 6 (proving the build-time check on the board crate) was skipped (commit 630f33a; plan).
