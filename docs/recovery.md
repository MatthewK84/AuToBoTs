# Recovery retry

The runtime does not send the recovery command once and hope it arrived. It keeps asking, on a bounded schedule, until the flight controller reports the mode in `HEARTBEAT`. The code is `crates/rta-host/src/recovery.rs`. The runtime wiring is in `docs/runtime.md`.

## One latch

There is one recovery-in-progress latch per process. A Revert command from `decide` engages it on the tick thread. A stalled tick thread engages it from the watchdog. Once engaged, the latch holds Revert until process restart, and the log line carries `latched`.

The tick thread and the watchdog drive the same latch. Each takes the latch lock, asks it for the step that is due, sends that step, and records the send, all before releasing the lock. Both threads see one schedule, so the same attempt is never sent twice. The watchdog drives the latch only while the tick thread is stalled.

## Schedule

The first send goes out on the tick that engages the latch. After send `n`, the next send waits `recovery.retry_initial_ms * 2^(n-1)`, capped at `recovery.retry_cap_ms`. Every step of the fallback order gets `recovery.attempts_per_step` sends. The wait comes from the config, not from a copied number. The reviewed `config/example.toml` leaves these keys out, so it runs on the defaults, and its hash in `docs/do178/sci.md` is unchanged.

| Key | Default | Check at load |
| --- | --- | --- |
| `recovery.retry_initial_ms` | 500 | at least `tick.mode_change_latency_ms` |
| `recovery.retry_cap_ms` | 2000 | at least `retry_initial_ms` |
| `recovery.attempts_per_step` | 3 | 1 to 10 |

A send is due only on a tick or a watchdog poll, so it can go out late by up to one period. It never goes out early. The clock for a send is read under the latch lock, to poll, and again after the send, to record it; it is not the reading a tick took at its top. A tick held up between the two (a slow complex function, the evaluation wait, a descheduled thread) would otherwise stamp the send early and pull the next one early by the delay. An ack is timed under the latch lock the same way. A local socket error is not counted as an attempt, and that step stays due.

## Fallback order

| Configured | Step 1 | Step 2 | Step 3 |
| --- | --- | --- | --- |
| `rtl` | `MAV_CMD_DO_SET_MODE` AUTO_RTL | `MAV_CMD_NAV_RETURN_TO_LAUNCH` | `MAV_CMD_NAV_LAND` |
| `loiter` | `MAV_CMD_DO_SET_MODE` AUTO_LOITER | `MAV_CMD_NAV_RETURN_TO_LAUNCH` | `MAV_CMD_NAV_LAND` |
| `land` | `MAV_CMD_DO_SET_MODE` AUTO_LAND | `MAV_CMD_NAV_LAND` | none |

Each step is confirmed by the mode it asks for: AUTO_RTL for `NAV_RETURN_TO_LAUNCH`, AUTO_LAND for `NAV_LAND`. The latch also accepts any mode asked for earlier in the order. Every order ends on a land. No step asks for OFFBOARD or ACRO. `NAV_LAND` carries NaN in params 4 to 7, which means land at the current position with the heading unchanged.

The order moves to the next step when a step's sends are used up, or at once on `DENIED` or `UNSUPPORTED`.

## COMMAND_ACK

The receive thread reads `COMMAND_ACK` (id 77) from its raw payload. The common dialect in the pinned mavlink crate has no `MAV_RESULT_IN_PROGRESS`, and the typed decoder would drop that frame. An ack counts only when it comes from the flight controller's system and component (from its heartbeat header) and names the command currently in flight. An ack for an earlier step is stale and changes nothing.

| Result | Action | Log code |
| --- | --- | --- |
| `ACCEPTED` | Not a confirmation. Holds the next resend until one full wait after the ack, or the spec's `heartbeat_stale` limit after it if that is longer, so the vehicle has time to report the mode. PX4 reports a mode only in its 1 Hz `HEARTBEAT`; against PX4 SITL the heartbeat showed the new mode up to about 1 s after the ack, and a 500 ms hold resent a `DO_SET_MODE` that had already taken (`docs/sitl.md`). | `ack:accepted` |
| `IN_PROGRESS` | Same as accepted. | `ack:in_progress` |
| `TEMPORARILY_REJECTED` | Resend on the schedule. | `ack:temporarily_rejected` |
| `FAILED` | Resend on the schedule. | `ack:failed` |
| `CANCELLED` or another value | Resend on the schedule. | `ack:cancelled`, `ack:result_<n>` |
| `DENIED` | The next step in the order is due at once. | `ack:denied`, `fallback:denied` |
| `UNSUPPORTED` | The next step in the order is due at once. | `ack:unsupported`, `fallback:unsupported` |

Only a `HEARTBEAT` showing the mode confirms recovery. An ack never does.

## When the vehicle never confirms

If the whole order is used up and no `HEARTBEAT` has shown the mode, the latch stops sending. A `DENIED` or `UNSUPPORTED` on the last step also stops it. The state is `recovery_unconfirmed`. Revert stays latched until restart, and no setpoint is sent. The companion no longer streams offboard setpoints, so the flight controller's own offboard-loss failsafe (`docs/icd-fc.md`) is what acts. The log keeps `recovery_unconfirmed` on every tick. The operator sees it there and in the degraded-mode matrix (`docs/degraded.md`).

With the defaults and `rtl`, there are nine sends over about 13.5 s. The latch gives up about 15.5 s after the first send. The latch allocates nothing, and its state is a fixed set of counters.

Once the mode is confirmed, nothing more is sent, even if the flight controller later leaves the mode. A pilot who takes over is not fought.

## Log codes

Every tick after the latch engages adds these reason codes: `recovery_pending`, `recovery_confirmed`, or `recovery_unconfirmed`; `attempts:<n>`; and `step:set_mode`, `step:nav_rtl`, or `step:nav_land`. The last ack and any fallback reason are added when present. A tick that put a recovery frame on the wire also adds `sent:<step>`.

## Not shown

No PX4 SITL run of the retry or the fallback is recorded. The end-to-end tests use a fake flight controller on loopback. The PX4 handling of `NAV_LAND` with NaN params, and of a repeated `DO_SET_MODE` while a mode change is already in progress, was not checked against a real PX4. No hazard row is added for a lost or refused recovery command. Whether to add one, and how to score it, is an owner decision.
