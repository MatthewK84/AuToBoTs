# Startup and shutdown

Startup is config, spec load, log open, then socket. A failed step does not skip ahead to the socket. The link is not constructed if the spec did not load.

Shutdown is stop setpoints, send the recovery mode, flush the log, then exit. The mode command is on the sink before the flush returns.

| Phase | Order |
| --- | --- |
| Startup | config, spec load, log open, socket |
| Shutdown | stop setpoints, recovery mode, flush log, exit |

A spec load error is the acceptance case. The link stays unconstructed.

`rta-host` follows this order in `main.rs` and `runtime::run`. SIGINT or `--ticks N` starts the shutdown order. The loop is in `docs/runtime.md`.

## Gating

`startup.mode` decides when the runtime may command the flight controller at all. Commands means setpoints and the recovery mode. The companion `HEARTBEAT` always goes out. The code is `crates/rta-host/src/gate.rs`.

Ready means a flight-controller `HEARTBEAT` has been heard and the position fix is no older than `startup.ready_fix_age_ms`.

| `startup.mode` | Before ready | Disarmed | Armed |
| --- | --- | --- | --- |
| `armed` (production default) | monitor only | monitor only, log `disarmed` | full fail-closed behavior |
| `immediate` (bench, SITL) | monitor only, log `not_ready` | full fail-closed behavior once ready | full fail-closed behavior once ready |

Monitor only means every tick is evaluated and logged, and no setpoint and no recovery command is sent. The watchdog obeys the same gate. It acts only if the last tick was allowed to act and, in `armed` mode, the flight controller still reports armed. The shutdown recovery command obeys it too.

In `armed` mode, the arm state is `MAV_MODE_FLAG_SAFETY_ARMED` in the flight-controller `HEARTBEAT`.
- An arm seen before the runtime is ready logs `not_ready_at_arm`. Revert is applied at once with full fail-closed behavior, and it latches until process restart. This includes a first heartbeat that is already armed.
- An arm after readiness starts the existing behavior unchanged.
- A disarm goes back to monitor only. Latches already set stay set until restart.

In `immediate` mode, the arm state is ignored. Nothing is commanded until the runtime is ready, so a heartbeat that arrives before the first position no longer sends a recovery mode at boot. If `startup.ready_timeout_ms` passes first, the gate opens anyway, and the spec's Revert on the missing fix is sent.

If the runtime is not ready `startup.ready_timeout_ms` after start, every tick logs `ready_timeout` until it is ready. In `armed` mode that commands nothing while the vehicle is disarmed.

| Key | Default | Check at load |
| --- | --- | --- |
| `startup.mode` | `armed` | `armed` or `immediate` |
| `startup.ready_timeout_ms` | 30000 | at least 1 |
| `startup.ready_fix_age_ms` | 200 | at least `tick.period_ms` |

`config/example.toml` sets `armed` explicitly. `config/sitl.toml` sets `immediate`. The degraded rows are in `docs/degraded.md`.
