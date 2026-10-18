# Degraded modes

What still flies when one input is lost. Commit is never allowed on a degraded row.

| Row | Navigation | Commit | Recovery |
| --- | --- | --- | --- |
| Lost tracker | allowed | not allowed | not commanded |
| Lost grant | allowed | not allowed | not commanded |
| Shadow only | allowed | not allowed | not commanded |
| Filter timeout | not allowed | not allowed | commanded |
| Stale link | not allowed | not allowed | commanded |
| Recovery unconfirmed | not allowed | not allowed | commanded until the order is used up |
| Disarmed | not allowed | not allowed | not commanded |
| Not ready | not allowed | not allowed | not commanded |
| Not ready at arm | not allowed | not allowed | commanded |
| Companion abort | not allowed | not allowed | not commanded by the companion |

Lost tracker and lost grant are inhibit. Setpoints may continue. Commit does not. Shadow only keeps the shadow proposal and drops commit. Filter timeout and a stale link are revert. Recovery is commanded.

Recovery unconfirmed means the flight controller never confirms the recovery mode in `HEARTBEAT`. The latch resends on the backoff and down the fallback order in `docs/recovery.md`. When the order is used up, it stops sending and Revert stays latched until restart. No setpoint is sent, so the flight controller's own offboard-loss failsafe acts. Every tick logs `recovery_unconfirmed`.

Disarmed, under `startup.mode = "armed"`, is monitor only. Every tick is evaluated and logged with `disarmed`, and nothing is sent, the watchdog included. Not ready is the same before the first heartbeat and fresh fix, and logs `not_ready` in `immediate` mode. Not ready at arm is an arm seen before readiness. It is full fail-closed Revert at once, latched until restart, and logs `not_ready_at_arm`. The gate is in `docs/startup.md`.

Companion abort is the host process ending on a panic in a release build, which aborts (`docs/panics.md`). The companion sends nothing more. In OFFBOARD the flight controller's offboard-loss failsafe acts; a vehicle already in a recovery mode stays in it. A restart is a fresh start under the startup gate.

The matrix matches the verdict tests. Inhibit clears commit. Revert has no commit field.
