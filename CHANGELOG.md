# Changelog

All notable changes to this repository. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). No version has been tagged yet; every crate is `0.1.0`. Entries name the pull request that made the change.

## [Unreleased]

### Fixed

- PX4 recovery-mode encoding: `DO_SET_MODE` sends AUTO with the RTL, loiter, or land sub-mode, and `HEARTBEAT.custom_mode` is decoded as PX4 packs it (#195).
- `decide_input` fails closed on non-finite energy, proved with Kani (#201).
- Setpoints are position and yaw only (`type_mask` `0x09F8`), with yaw passed through, `time_boot_ms` from the monotonic run clock, and the target system and component from the heard flight-controller heartbeat. `COMMAND_ACK` counts only from that system and component (#204).
- The runtime tests use an 80 ms deadline, so a loaded runner cannot make the first tick late (#205).

### Added

- The integrated runtime loop: a bound UDP link, a receive thread, a fixed-rate tick, and an independent watchdog (#196).
- A preemptive tick deadline: evaluation runs on a worker thread behind a bounded handoff, and a hung evaluator reverts within one deadline (#197).
- Recovery is resent on a bounded schedule until the flight controller reports the mode, with RTL and land fallbacks. `COMMAND_ACK` is read, `IN_PROGRESS` included (#198).
- Startup gating on arm state and readiness: monitor only while disarmed, and a latched `not_ready_at_arm` fault (#199).
- Property tests for the parsing and decision paths, and cargo-fuzz targets for `read::apply`, MAVLink frames and the raw `COMMAND_ACK` reader, and config parsing, with a CI fuzz smoke job (#205).
- CI jobs: a release build, an aarch64 (Jetson) cross-build, and a SITL smoke run against the stand-in peer (#206).
- `LICENSE` (Apache-2.0), `CONTRIBUTING.md`, this changelog, and `.gitignore`.
- Process diagnostics from `rta-host` are single lines with a level and a component. `RTA_LOG` sets the lowest level shown (`docs/runtime.md`).

### Changed

- The host's heartbeat limit is read from the spec, and every latch reaches the switch (#200).
- The per-tick path is bounded: enum reason codes, a fixed log-line buffer, and fixed rings for the fail-closed notes and status text. A steady-state tick allocates nothing on the tick thread. The tick log stays byte-identical (#202).
- `rta-host`'s library and binaries deny `unwrap`, `expect`, indexing, and explicit panics outside tests. A release build aborts on panic, and the flight controller's offboard-loss failsafe takes over (#203).
- Document and source-text checks moved out of `rta-host` into the new `rta-docs` crate, which no binary links (#205).
- CI actions are pinned to commit SHAs, tokens are read-only except closing an issue, and every crate declares `rust-version = "1.98.1"` (#206).
- The `rta-host` start-up error and run summary lines on stderr and stdout changed format, for example from `config: <error>` to `rta-host level=error component=config msg="<error>"`. The tick log is unchanged.
