# MAVLink

Dialect is common, pinned in M0.2. The link address is `udpout:host:port` or `serial:path:baud` from `link.endpoint`.

Outbound heartbeat is `HEARTBEAT` at 1 Hz. System id and component id come from config. The component type is onboard controller, not a flight controller.

Inbound messages this adapter already decodes:

- `HEARTBEAT` updates the flight-controller heartbeat age from local receive time.
- `GLOBAL_POSITION_INT` updates altitude. The sender timestamp is not an age.
- `STATUSTEXT` is logged and does not change the record.

`send` is the only outbound path. Later mode and setpoint issues add messages here. They do not open a second socket.
