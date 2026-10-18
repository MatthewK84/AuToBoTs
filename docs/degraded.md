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

Lost tracker and lost grant are inhibit. Setpoints may continue. Commit does not. Shadow only keeps the shadow proposal and drops commit. Filter timeout and a stale link are revert. Recovery is commanded.

Recovery unconfirmed means the flight controller never confirms the recovery mode in `HEARTBEAT`. The latch resends on the backoff and down the fallback order in `docs/recovery.md`. When the order is used up, it stops sending and Revert stays latched until restart. No setpoint is sent, so the flight controller's own offboard-loss failsafe acts. Every tick logs `recovery_unconfirmed`.

The matrix matches the verdict tests. Inhibit clears commit. Revert has no commit field.
