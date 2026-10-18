# Flight-controller interface

Dialect is `common`, frozen in M0.2. The same name is in `docs/dependencies.md` and `docs/mavlink.md`. ArduPilot is not linked.

This is the contract for the one link the adapter owns. There is no second socket.

## Messages the adapter sends

| Message | Id | Rate | Units | When |
| --- | --- | --- | --- | --- |
| `HEARTBEAT` | 0 | 1 Hz | none | Always. Component type is onboard controller, not a flight controller. |
| `COMMAND_LONG` with `MAV_CMD_DO_SET_MODE` | 76 | on mode change, resent on the backoff in `docs/recovery.md` until the mode is reported | param1 custom-mode flag, param2 PX4 main mode, param3 PX4 sub-mode, `confirmation` counts resends | `SwitchCommand::Mode`. Idle sends nothing. |
| `COMMAND_LONG` with `MAV_CMD_NAV_RETURN_TO_LAUNCH` (20) | 76 | fallback only | no params | `DO_SET_MODE` denied, unsupported, or used up. |
| `COMMAND_LONG` with `MAV_CMD_NAV_LAND` (21) | 76 | last fallback | params 4 to 7 NaN: here, heading unchanged | Every fallback order ends on it. |
| `SET_POSITION_TARGET_LOCAL_NED` | 84 | tick rate | north, east, down in meters | `SwitchCommand::Setpoints`. |
| `STATUSTEXT` | 253 | on indication change | text, 50 chars | The indication. It does not change the record. |

## Messages the adapter reads

| Message | Id | Effect |
| --- | --- | --- |
| `HEARTBEAT` | 0 | Resets `fc_heartbeat_age_ms` from local receive time. |
| `GLOBAL_POSITION_INT` | 33 | Updates `alt_m` in meters from millimeters. Resets `fix_age_ms`. The sender timestamp is not an age. Not read by the runtime loop. |
| `LOCAL_POSITION_NED` | 32 | Runtime loop fix: north `x` and east `y` in meters for the host fence, altitude `-z`. Resets `fix_age_ms` when all three are finite. |
| `STATUSTEXT` | 253 | Logged. Does not change the record or the verdict. |
| `SYS_STATUS` | 1 | Logged. Not a predicate. Battery and estimator fields are not read. |
| `COMMAND_ACK` | 77 | Read from the raw payload, so `IN_PROGRESS` (5) is kept. From the flight controller's system id only. Drives the recovery latch per `docs/recovery.md`. Never confirms a mode; only `HEARTBEAT` does. |

An unknown inbound message is dropped. It does not change an age. A `HEARTBEAT` whose autopilot is `MAV_AUTOPILOT_INVALID` is a ground station or another companion. It does not reset `fc_heartbeat_age_ms` or the reported mode.

The runtime loop and its endpoint forms are in `docs/runtime.md`.

## Mode enum

PX4 custom mode from `src/modules/commander/px4_custom_mode.h`. Every recovery mode is a sub-mode of the AUTO main mode. RTL, loiter, and land are not main modes.

`COMMAND_LONG` `MAV_CMD_DO_SET_MODE` carries:

- param1: `MAV_MODE_FLAG_CUSTOM_MODE_ENABLED` (1).
- param2: PX4 main mode. Always `PX4_CUSTOM_MAIN_MODE_AUTO` (4).
- param3: PX4 sub-mode from the table below.

| Recovery | Main mode | Sub-mode | Reported `HEARTBEAT.custom_mode` |
| --- | --- | --- | --- |
| `Rtl` | AUTO (4) | `PX4_CUSTOM_SUB_MODE_AUTO_RTL` (5) | `0x05040000` |
| `Loiter` | AUTO (4) | `PX4_CUSTOM_SUB_MODE_AUTO_LOITER` (3) | `0x03040000` |
| `Land` | AUTO (4) | `PX4_CUSTOM_SUB_MODE_AUTO_LAND` (6) | `0x06040000` |

PX4 packs `HEARTBEAT.custom_mode` as a union: bits 0 to 15 reserved, bits 16 to 23 `main_mode`, bits 24 to 31 `sub_mode`. The adapter decodes main = `(custom_mode >> 16) & 0xFF` and sub = `(custom_mode >> 24) & 0xFF`. Only AUTO with the RTL, loiter, or land sub-mode is the matching recovery. Any other value is not a recovery mode. That includes OFFBOARD (main 6), ACRO (main 5), and other AUTO sub-modes such as mission or takeoff.

The constants are in `crates/rta-host/src/command_out.rs`. A bare main mode of 6, 5, or 4 is not a recovery mode. On PX4, main mode 6 is OFFBOARD and 5 is ACRO.

## Companion death

If the companion dies, the flight controller's own failsafe runs. This is not a companion behavior and not a hope.

On the PX4 tree used as the reference, offboard loss is `Failsafe::fromOffboardLossActParam`. The configured action is one of position, altitude, stabilized, return, land, hold, terminate, or disarm. The default is position mode. The timeout is the flight controller's offboard-loss parameter, not a value this repository invents.

The companion cannot fly the recovery after it has died. The switch selects a mode while it is alive. After it is dead, the flight controller does.
