# SITL harness

The stack is PX4 SITL. The onboard MAVLink port is `udp:127.0.0.1:14540`, the same endpoint as `config/example.toml` and `config/sitl.toml`.

`scripts/sitl.sh` checks that those endpoints match, then brings the port up. If `PX4_DIR` points at a built PX4 tree it starts that binary. Otherwise it binds the same port with `scripts/sitl_peer.py` so the host can still connect and send a heartbeat. The host is `rta-sitl`, which loads the SITL config, opens the example endpoint, sends one heartbeat, and logs a nominal hover.

The example polygon is one meter on a side, smaller than the stopping margin at the configured speed. The SITL fence is a 200 meter square with the same speed, acceleration, and latency, so the hover point is inside the margin. Ages are 10 ms, under the spec limits. The script fails unless `sitl.log` contains `verdict=pass`. The log and `sitl-timing.txt` carry the hash of the spec `rta-sitl` loaded, not zero. The hover verdict goes through `FailClosed` with the spec's heartbeat limit, the deadline, and the write latch. The line carries `not_sent` because the hover sends no command. A failed log write fails the run.

## Scripted trips

`scripts/sitl-trips.sh` runs three cases and reads the tick log. A ground station is not the check.

- Fence breach writes `fc_report=rtl` and `verdict=revert` with `command=mode`.
- A stopped heartbeat writes `verdict=revert`.
- A weak track inside commit range writes `verdict=inhibit` and `commit=0`. The log must not contain a commit command.

## Real PX4 SITL

`scripts/sitl-px4.sh` runs `rta-host` itself, not `rta-sitl`, against real PX4 software in the loop. It uses PX4-Autopilot `v1.16.2`, pinned to commit `54f0455ffcd755534539a7cf33a09a20bf71d29d`, built as `px4_sitl_default` and run headless with PX4's built-in SIH quadrotor (`sihsim_quadx`, `PX4_SYS_AUTOSTART=10040`). There is no Gazebo. The script clones and builds PX4 once under `PX4_HOME` (default `~/.cache/rta-px4`; about 4 minutes from scratch on an 8-core host) and checks the commit. Then `scripts/sitl_px4.py` runs each scenario against a fresh PX4 instance:

- `rta-host` runs with `udpin:127.0.0.1:14540` on PX4's onboard port and `startup.mode = "armed"`.
- The vehicle is driven from the PX4 shell (`px4-commander arm`, `px4-commander takeoff`, `px4-param set MIS_TAKEOFF_ALT 10`).
- What PX4 reports is recorded on the ground-station link (port 14550): `HEARTBEAT` `custom_mode` and arm state, altitude, `COMMAND_ACK`.

The evidence is one JSON per scenario in `crates/rta-host/fixtures/px4-sitl/`. Each is labelled `"evidence": "px4-sitl"` and `"hardware": false`. **This is PX4 SITL, not hardware and not a flight test.** SIH is PX4's own simplified dynamics model.

Results, 2026-10-22, PX4 v1.16.2 SITL:

| Scenario | Setup | What PX4 did | rta-host |
| --- | --- | --- | --- |
| `gate` | 200 m square fence, 120 m cap; arm and take off after 5 s disarmed | Reported AUTO.LOITER disarmed, armed, AUTO.TAKEOFF (`0x02040000`), then AUTO.LOITER (`0x03040000`) at 9.9 m. No mode change from rta-host | 99 of 99 disarmed ticks gated (`disarmed`, or `no_fc` before the first heartbeat), no command. Armed: `no_request`, no recovery sent |
| `rtl` | Vertical cap 5 m, takeoff to 10 m, `recovery.mode = "rtl"` | Entered AUTO.RTL, `HEARTBEAT.custom_mode` `0x05040000`, at 5.65 m | Breach on tick 287, one `DO_SET_MODE`, `ack:accepted`, `recovery_confirmed` on tick 297 from the heartbeat |
| `loiter` | As `rtl`, `recovery.mode = "loiter"` | Left AUTO.TAKEOFF for AUTO.LOITER (`0x03040000`) at 5.63 m and held | One `DO_SET_MODE`, accepted, confirmed |
| `land` | As `rtl`, `recovery.mode = "land"` | Entered AUTO.LAND (`0x06040000`) at 5.63 m | One `DO_SET_MODE`, accepted, confirmed |
| `late_start` | rta-host started while armed and holding at 10 m: a companion restart in flight | Stayed in AUTO.LOITER | About 1 s of `no_fc` until the first heartbeat, with the position already fresh, so ready at the first armed heartbeat and no `not_ready_at_arm`. Nothing sent |
| `probe` | Commands sent from the ground-station link with rta-host's params | See below | not running |

What the probe showed, and how it differs from the fake flight controller in `runtime.rs`:

- PX4's `HEARTBEAT` comes every 1.020 to 1.041 s on the onboard link. The heartbeat age the tick saw peaked at 1019 to 1035 ms in every run.
- A new mode shows in `HEARTBEAT` 0.32 to 1.0 s after PX4 accepts the command. The fake adopts a commanded mode on its next heartbeat at 10 Hz.
- PX4 answers every valid `DO_SET_MODE` with `ACCEPTED` (0), twice on the ground-station link. Two back-to-back `DO_SET_MODE` RTL (confirmation 0 and 1) were both accepted, and so was a third while already in RTL. A mode change in progress is not refused.
- An unknown sub-mode (main 4, sub 99) got `ACCEPTED` and then `TEMPORARILY_REJECTED` (1) for the same command, and the mode did not change. rta-host never sends one.
- `NAV_LAND` with NaN yaw, latitude, longitude and altitude (the land step) was accepted and PX4 entered AUTO.LAND. `NAV_RETURN_TO_LAUNCH` with zero params was accepted and PX4 entered AUTO.RTL.
- PX4 streams `LOCAL_POSITION_NED` before its first `HEARTBEAT` reaches the host (about 0.5 to 1 s), so position-before-armed-flag is the order seen at startup and on a late start.
- PX4 reports AUTO.LOITER while disarmed on the ground.

Two defects were found and fixed with this harness:

- The spec's `heartbeat_stale` limit was 1000 ms, one heartbeat period. The age passed it on PX4's normal 1 Hz stream, so the spec reverted every two seconds. Once armed, the first such tick latched recovery and PX4 went to AUTO.RTL 2 s after takeoff at 0.2 m. The record is `fixtures/px4-sitl/gate-heartbeat-limit-1000ms.json`. The limit is now 1500 ms (`docs/spec-notes.md`).
- After an `ACCEPTED` ack the next resend waited only `retry_initial_ms` (500 ms), shorter than the heartbeat period, so rta-host resent a `DO_SET_MODE` that had already taken (ticks 153 and 164 in that record). The hold after an accepting ack is now at least the `heartbeat_stale` limit (`docs/recovery.md`).

The fallback steps (`NAV_RETURN_TO_LAUNCH`, `NAV_LAND`) went out from the probe, not from rta-host. PX4 accepts `DO_SET_MODE`, so the latch never falls back against it.

CI job `px4-sitl` runs the script on every push and pull request.

## Timing

SITL timing is not target timing. The SITL log header carries that disclaimer, and the eval-duration distribution is written to `sitl-timing.txt`. Those numbers are a host-side sample, not a board measurement.

## Claims

SITL is developmental. A test name is cited only in its own column.

| Column | Item |
| --- | --- |
| Shown | `scripts/sitl.sh`. Mode change on fence breach, in simulation. |
| Shown | `scripts/sitl-trips.sh`. Inhibit on weak track. Revert on heartbeat loss. In simulation. |
| Shown | `scripts/sitl-px4.sh`. Startup gate, fence breach to AUTO.RTL, AUTO.LOITER and AUTO.LAND, confirmed from PX4's heartbeat. In PX4 SITL v1.16.2 with SIH, not hardware. |
| Not shown | Aerodynamic truth. |
| Not shown | Latency on the target. Target timing. |
| Not shown | Tracker performance. |
