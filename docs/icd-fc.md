# Flight-controller interface

Dialect is `common`, frozen in M0.2. The same name is in `docs/dependencies.md` and `docs/mavlink.md`. ArduPilot is not linked.

This is the contract for the one link the adapter owns. There is no second socket.

## Messages the adapter sends

| Message | Id | Rate | Units | When |
| --- | --- | --- | --- | --- |
| `HEARTBEAT` | 0 | 1 Hz | none | Always. Component type is onboard controller, not a flight controller. |
| `COMMAND_LONG` with `MAV_CMD_DO_SET_MODE` | 76 | on mode change | custom mode is a PX4 main mode | `SwitchCommand::Mode`. Idle sends nothing. |
| `SET_POSITION_TARGET_LOCAL_NED` | 84 | tick rate | north, east, down in meters | `SwitchCommand::Setpoints`. |
| `STATUSTEXT` | 253 | on indication change | text, 50 chars | The indication. It does not change the record. |

## Messages the adapter reads

| Message | Id | Effect |
| --- | --- | --- |
| `HEARTBEAT` | 0 | Resets `fc_heartbeat_age_ms` from local receive time. |
| `GLOBAL_POSITION_INT` | 33 | Updates `alt_m` in meters from millimeters. Resets `fix_age_ms`. The sender timestamp is not an age. |
| `STATUSTEXT` | 253 | Logged. Does not change the record or the verdict. |
| `SYS_STATUS` | 1 | Logged. Not a predicate. Battery and estimator fields are not read. |

An unknown inbound message is dropped. It does not change an age.

## Mode enum

PX4 custom main mode, sent in `COMMAND_LONG` param2.

| Recovery | Custom mode |
| --- | --- |
| `Rtl` | 6 |
| `Loiter` | 5 |
| `Land` | 4 |

A reported custom mode of 6, 5, or 4 is the matching recovery. Any other value is not a recovery mode.

## Companion death

If the companion dies, the flight controller's own failsafe runs. This is not a companion behavior and not a hope.

On the PX4 tree used as the reference, offboard loss is `Failsafe::fromOffboardLossActParam`. The configured action is one of position, altitude, stabilized, return, land, hold, terminate, or disarm. The default is position mode. The timeout is the flight controller's offboard-loss parameter, not a value this repository invents.

The companion cannot fly the recovery after it has died. The switch selects a mode while it is alive. After it is dead, the flight controller does.
