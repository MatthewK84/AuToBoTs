# Interface

Position is `GLOBAL_POSITION_INT`. Heartbeat is `HEARTBEAT`. Both are MAVLink common. Age is the host receive time. Sender timestamps are not used.

The bench tracker is not a MAVLink message. It is a separate port. A tracker sample updates confidence and range only. It does not update the fix timestamp.

`STATUSTEXT` is logged. It does not change the record and it does not change the verdict.

The unit-test byte blob is a stand-in: `1` plus a little-endian altitude in millimeters, `2` for a heartbeat, `3` plus the status text. The dialect decode replaces that blob. It does not change the field rules.

`link_age_ms` is time since the last received message from the configured endpoint, excluding `STATUSTEXT`. The bench has no separate command stream, so a position or a heartbeat counts. The age is updated at the start of the tick, before the spec runs. No message yet is an age larger than the spec limit, so the first verdict is `Revert`.
