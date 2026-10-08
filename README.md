# AuToBoTs

Fail-closed runtime assurance for Group 1-3 aircraft. The flight controller owns actuators and recovery. A Rust host owns I/O and the switch. An RTLola specification owns every predicate. The complex function never reaches the MAVLink socket.

This repository bounds flight and commit. It does not assure a tactical decision, and it is not an authorizing-official package. It does not implement a weapon, a fuze, or a release actuator. Thresholds live in the specification, not in Rust. A fault maps to Revert. A latched interpreter fault clears only on process restart.

Task list: [docs/engineering-tasks.md](docs/engineering-tasks.md). Standards gate: [docs/standards-3000-09-8430-01.md](docs/standards-3000-09-8430-01.md).

## Crates

- `rta-spec` — input record and verdicts. No sockets.
- `rta-switch` — pure command decision. No interpreter, no MAVLink.
- `rta-host` — binary. Config, clock, interpreter, link, log, watchdog.
- `rta-replay` — re-evaluates a tick log against the spec.

Toolchain pin: Rust 1.98.1 with rustfmt and clippy, in `rust-toolchain.toml`.
