# Changelog

All notable changes to this repository. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). No version has been tagged yet; every crate is `0.1.0`. Entries name the pull request that made the change.

## [Unreleased]

### Fixed

- Stress failures classified (`docs/stress.md`). Two were whole-host pauses (VM snapshot and resume; `CLOCK_MONOTONIC` jumped seconds): every runtime test now runs under `host_pause::guard`, which reruns a test that failed across a gap of 1 s or more and reports `host paused`. One was a test assumption: under load the arm in `arm_after_ready_then_a_breach_sends_one_auto_rtl_and_disarm_is_monitor_only` could land on a stale fix, where `not_ready_at_arm` is correct; the test now checks that path and tries again for a ready arm. `scripts/sitl_px4.py` tells an `rta-host` deadline miss or a starved PX4 simulator from a result and runs that scenario once more. `scripts/stress.sh` added. No product change; the 5 ms deadline is unchanged.
- `heartbeat_stale` is 1500 ms, not 1000 ms. PX4's 1 Hz onboard heartbeat reached an age of 1014 to 1035 ms between healthy beats, so the spec reverted a healthy vehicle; against PX4 SITL it sent RTL 2 s after takeoff.
- After an `ACCEPTED` or `IN_PROGRESS` ack the next resend waits at least the `heartbeat_stale` limit, since PX4 reports the new mode only in its 1 Hz heartbeat. A 500 ms hold resent a `DO_SET_MODE` that had already taken.
- PX4 recovery-mode encoding: `DO_SET_MODE` sends AUTO with the RTL, loiter, or land sub-mode, and `HEARTBEAT.custom_mode` is decoded as PX4 packs it (#195).
- `decide_input` fails closed on non-finite energy, proved with Kani (#201).
- Setpoints are position and yaw only (`type_mask` `0x09F8`), with yaw passed through, `time_boot_ms` from the monotonic run clock, and the target system and component from the heard flight-controller heartbeat. `COMMAND_ACK` counts only from that system and component (#204).
- The runtime tests use an 80 ms deadline, so a loaded runner cannot make the first tick late (#205).
- A recovery resend is timed from the clock read under the latch lock, after the send, not from the reading at the top of the tick. A tick held up before it sent could pull the next resend early by the delay; the stress run saw one 333 ms after the last against a 600 ms wait.
- `rta-target` no longer names a board it did not run on. Its line carries `evidence` (`simulated`, `host-standin`, `real-target`) and `measured_on`; the 2026-10-09 fixtures are relabelled `evidence=host-standin`.

### Added

- The integrated runtime loop: a bound UDP link, a receive thread, a fixed-rate tick, and an independent watchdog (#196).
- A preemptive tick deadline: evaluation runs on a worker thread behind a bounded handoff, and a hung evaluator reverts within one deadline (#197).
- Recovery is resent on a bounded schedule until the flight controller reports the mode, with RTL and land fallbacks. `COMMAND_ACK` is read, `IN_PROGRESS` included (#198).
- Startup gating on arm state and readiness: monitor only while disarmed, and a latched `not_ready_at_arm` fault (#199).
- Property tests for the parsing and decision paths, and cargo-fuzz targets for `read::apply`, MAVLink frames and the raw `COMMAND_ACK` reader, and config parsing, with a CI fuzz smoke job (#205).
- CI jobs: a release build, an aarch64 (Jetson) cross-build, and a SITL smoke run against the stand-in peer (#206).
- Simulated Jetson Orin Nano 8GB eval timing: `scripts/target-sim.sh` runs the aarch64 `rta-target` under qemu user mode and a scaled host model, 10,000 ticks each, with percentiles and a histogram, labelled `evidence=simulated`. `docs/target-timing.md` gives the assumptions and the board procedure. A CI `target-sim` smoke job.
- `scripts/sitl-px4.sh`: `rta-host` against real PX4 v1.16.2 SITL (SIH, headless), with the startup gate, fence breach to RTL, loiter and land, a late start, and a probe of PX4's ACKs. Evidence in `crates/rta-host/fixtures/px4-sitl`, labelled PX4 SITL, not hardware. CI job `px4-sitl`.
- CI `px4-sitl`: installs the pinned toolchain once (`rustup toolchain install 1.98.1 --profile minimal`), and `scripts/sitl-px4.sh` builds with `RUSTUP_TOOLCHAIN` from the repo root, so rustup adds no `rust-toolchain.toml` components there and cannot collide with a preinstalled `cargo-fmt`. The PX4 pin is checked against the annotated tag peeled to its commit (`v1.16.2^{commit}`) as well as `HEAD`.
- `LICENSE` (Apache-2.0), `CONTRIBUTING.md`, this changelog, and `.gitignore`.
- Process diagnostics from `rta-host` are single lines with a level and a component. `RTA_LOG` sets the lowest level shown (`docs/runtime.md`).

### Changed

- The host's heartbeat limit is read from the spec, and every latch reaches the switch (#200).
- The per-tick path is bounded: enum reason codes, a fixed log-line buffer, and fixed rings for the fail-closed notes and status text. A steady-state tick allocates nothing on the tick thread. The tick log stays byte-identical (#202).
- `rta-host`'s library and binaries deny `unwrap`, `expect`, indexing, and explicit panics outside tests. A release build aborts on panic, and the flight controller's offboard-loss failsafe takes over (#203).
- Document and source-text checks moved out of `rta-host` into the new `rta-docs` crate, which no binary links (#205).
- CI actions are pinned to commit SHAs, tokens are read-only except closing an issue, and every crate declares `rust-version = "1.98.1"` (#206).
- The `rta-host` start-up error and run summary lines on stderr and stdout changed format, for example from `config: <error>` to `rta-host level=error component=config msg="<error>"`. The tick log is unchanged.
