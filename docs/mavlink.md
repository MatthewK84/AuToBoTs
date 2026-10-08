# MAVLink

Dialect is common, pinned in M0.2. The link address is `udpout:host:port` or `serial:path:baud` from `link.endpoint`.

Outbound heartbeat is `HEARTBEAT` at 1 Hz. System id and component id come from config. The component type is onboard controller, not a flight controller.

Inbound messages this adapter already decodes:

- `HEARTBEAT` updates the flight-controller heartbeat age from local receive time.
- `GLOBAL_POSITION_INT` updates altitude. The sender timestamp is not an age.
- `STATUSTEXT` is logged and does not change the record.

`send` is the only outbound path. Later mode and setpoint issues add messages here. They do not open a second socket.

Message ids for the common dialect:

- `HEARTBEAT` is 0. It drives `fc_heartbeat_age_ms` only.
- `GLOBAL_POSITION_INT` is 33. It drives `alt_m` and resets `fix_age_ms`. The sender `time_boot_ms` is not an age.
- `STATUSTEXT` is 253. It is logged.
- `SYS_STATUS` is 1. It is logged in v1. It is not a predicate. The adapter does not read battery or estimator fields from it.

`link_age_ms` is the age of the companion socket. A position or a heartbeat counts. A status text does not. It is separate from `fc_heartbeat_age_ms` because the two fail differently.

Outbound commands:

- `SwitchCommand::Mode` is `COMMAND_LONG` with `MAV_CMD_DO_SET_MODE`, message id 76. `SET_MODE` is superseded. The bench custom mode is 6 for rtl, 5 for loiter, and 4 for land.
- `SwitchCommand::Setpoints` is `SET_POSITION_TARGET_LOCAL_NED`, message id 84.
- `SwitchCommand::Idle` sends nothing. A second revert tick sends nothing when the reported custom mode already matches recovery.
