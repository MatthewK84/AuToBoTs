# Runtime

`rta-host <config.toml> [--ticks N]` runs the loop in `crates/rta-host/src/runtime.rs`. Startup is config, spec load, log open, then socket, as in `docs/startup.md`. A failed step exits before the link is bound.

## Threads

| Thread | Does | Does not |
| --- | --- | --- |
| Tick | Ages samples, builds the `InputRecord` with `try_new`, hands it to the evaluation worker and waits at most `tick.deadline_ms`, passes the verdict through `FailClosed` and `decide`, sends the outbound message, writes the tick log, publishes the tick. Sends the companion `HEARTBEAT` at 1 Hz. | Read the socket. |
| Receive | Reads datagrams with a 20 ms timeout and applies each MAVLink frame to the shared vehicle state with local receive time. Hands each flight-controller `COMMAND_ACK` to the recovery latch. | Call the interpreter or send. |
| Evaluation | Owns the RTLola monitor, built once at startup. Evaluates one record per request. Detail in `docs/deadline.md`. | Send, or hold any lock the tick thread takes. |
| Watchdog | Reads the published tick. After `tick.watchdog_misses` missed periods it engages the recovery latch and drives its resends on its own socket handle while the tick thread stays stalled. | Call the interpreter, the filter, or the tracker. Wait for the tick thread. |

The vehicle state is fixed size. A status text changes no field. The three threads share one monotonic clock that starts with the run.

## One tick

1. Snapshot the vehicle state. Ages are now minus local receive time. A sample never received is `f64::MAX`, which is stale.
2. `fence_ok` is the host fence on the `LOCAL_POSITION_NED` north and east, with altitude `-z`. No fix is `fence_ok = false`. `source = "fc"` is `fence_ok = false` in the host record.
3. No tracker is wired. Track confidence is zero at range zero, which is the lost-tracker inhibit in `docs/degraded.md`.
4. A record that `try_new` rejects is no verdict. `FailClosed` makes no verdict Revert.
5. The deadline is preemptive. The tick thread waits at most `tick.deadline_ms` for the evaluation worker and does not wait for a hung evaluator. A missed deadline, a busy worker, or a dead worker is Revert and latches as an evaluation fault until restart; the line carries `late` or `eval_fault`.
6. Recovery in progress, a failed log write, an evaluation fault, or an arm before readiness latches Revert until restart. The log line carries `latched`.
7. `decide` is the only constructor of the command. `command_out` encodes it.
8. Nothing is sent before the first flight-controller `HEARTBEAT`. The line carries `no_fc`. After it, the startup gate in `docs/startup.md` decides whether the tick may act. A monitor-only line carries `disarmed` or `not_ready`. An arm before readiness carries `not_ready_at_arm` and latches Revert.
9. A `Setpoints` command with no complex-function request is not forwarded. The switch fills a missing request with zeros, and a zero setpoint is a real position. The line carries `no_request`.
10. The tick log line names the verdict the switch acted on and the command `decide` returned.

The loop is paced by a monotonic deadline that advances one period per tick. An overrun restarts the schedule from now and is counted.

## Recovery

Recovery is resent until the flight controller reports it. A Revert command engages one recovery latch. The tick thread and the watchdog drive that latch under one lock, so they never double-send. It resends on a bounded backoff, reads `COMMAND_ACK`, and falls back from `DO_SET_MODE` to `NAV_RETURN_TO_LAUNCH` and then `NAV_LAND` on `DENIED` or `UNSUPPORTED`. When the order is used up unconfirmed, it stops sending and stays latched. The policy, the ack table, and the log codes are in `docs/recovery.md`.

`decide` takes a reported mode, not an option. A flight controller in no recovery mode is passed as a recovery mode other than the configured one, so Revert still sends.

## Shutdown

SIGINT or `--ticks N` ends the loop. The order is stop setpoints, send the recovery step that is due unless the startup gate holds, the flight controller already reports the mode, or the latch is confirmed or used up, flush the log, stop the evaluation worker, join the receive and watchdog threads, exit 0. A faulted worker may be hung, so it is detached, not joined.

## Endpoints

| Form | Binds | Sends to | Keeps |
| --- | --- | --- | --- |
| `udp:host:port` | an ephemeral local port | `host:port` | datagrams from `host:port` only |
| `udpin:host:port` | `host:port` | the most recent sender | every sender |

`udpin` is the form a PX4 SITL onboard port expects. `config/example.toml` keeps `udp:` so `scripts/sitl.sh` and its stand-in peer are unchanged.

## Not shown

No PX4 SITL run of this loop is recorded. The end-to-end tests use a fake flight controller on loopback.
