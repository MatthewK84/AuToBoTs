# AuToBoTs

Fail-closed runtime assurance for Group 1–3 aircraft.

The flight controller owns actuators and recovery. A Rust host owns I/O and the switch. An RTLola specification owns every predicate. The complex function never reaches the MAVLink socket.

This repository bounds flight and commit. It does not assure a tactical decision, and it is not an authorizing-official package.

## Layout

- `docs/engineering-tasks.md` — one GitHub issue per heading, with the work and the acceptance criteria.
- `crates/rta-spec` — input record and verdicts. No sockets.
- `crates/rta-switch` — pure command decision. No interpreter, no MAVLink.
- `crates/rta-host` — binary. Config, interpreter, link, log, watchdog.
- `crates/rta-replay` — re-evaluates a tick log against the spec.

## Frozen rules

- Thresholds live in `spec/monitor.lola`, not in Rust.
- Late tick, interpreter error, missing verdict, NaN input, dead heartbeat, and an in-flight log failure all map to `Revert`.
- A latched interpreter fault clears only on process restart.
- Recovery is RTL, Loiter, or Land, chosen in config, executed by the flight controller.

## Status

Task list only. Implementation starts at M0.1.

The milestone list is `docs/milestone-list.md`. M0–M39 are in `docs/engineering-tasks.md`. M20–M39 are the twenty added after the expanded-scope work, and each has a GitHub milestone. Recovery the switch trusts remains the flight controller. Arming circuits, fuzes, safe-and-arm devices, and release actuators stay out of this repository.
