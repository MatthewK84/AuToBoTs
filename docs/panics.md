# Panics

The host library and every host binary deny the panicking constructs outside tests: `clippy::unwrap_used`, `clippy::expect_used`, `clippy::indexing_slicing`, `clippy::panic`, `clippy::unreachable`, `clippy::todo`, and `clippy::unimplemented`. Each site that used one now returns an error or takes the fail-closed value instead:

| Site | Was | Now |
| --- | --- | --- |
| `event.rs` | `expect` on a non-finite float | `None`, which the evaluator treats as an interpreter fault: Revert, latched |
| `read.rs` | slice and `unwrap` on the position blob | `get` and `ReadError::Unknown` |
| `fence.rs` | indexed polygon edges | an edge iterator; fewer than three vertices is still never inside |
| `learning_hazards.rs` | `expect` on a missing row | Revert, the fail-closed outcome; a test still requires every row |
| `command_out.rs` | sliced status-text copy | a bounded `zip` |
| `sitl.rs` | indexed percentiles | `get`, `first`, and `last` |
| `bin/rta-target.rs` | `expect` on config, spec, and hover record; indexed `/proc` fields | exit 1 with a message; a missing field reads as zero |
| `bin/rta-trips.rs` | `expect` on the trip records | exit 1 with a message |

Lints are not a proof. Arithmetic overflow, a panic inside a dependency such as the RTLola interpreter or the MAVLink crate, and a failed allocation are not caught by them.

## Release builds abort

The workspace release profile sets `panic = "abort"`. A panic that still happens in a release build ends the process at once. Nothing unwinds, no thread carries on with half-updated state, and no destructor runs. The tick log keeps what was already written; a line not yet flushed is lost. Debug and test builds keep unwinding, so the tests that kill the evaluation worker still see `Gone`. In a release build, a panic on the evaluation worker aborts the whole process, so the latched `Gone` path in `docs/deadline.md` is not reached.

## What the vehicle does

When the companion process dies, the companion `HEARTBEAT` and every setpoint stop. Nothing on the companion commands recovery after that.

- **In OFFBOARD**, the flight controller's own offboard-loss failsafe acts once its offboard-loss timeout passes (`docs/icd-fc.md`). The action and the timeout are flight-controller parameters, not values this repository sets.
- **In a recovery mode already** (AUTO_RTL, loiter, or land, after a Revert), the flight controller stays in that mode.
- **Disarmed**, nothing happens.

## Restart

The host does not restart itself. A supervisor may restart it; the unit file is a deployment choice and is not in this repository. A restart is a fresh start (`docs/startup.md`): config, spec, log, then link. Every latch starts clear, and the startup gate applies. Under `startup.mode = "armed"` the first flight-controller `HEARTBEAT` after a mid-flight restart is already armed. If it arrives before a fresh fix, that is `not_ready_at_arm`: Revert is latched and recovery is commanded.
