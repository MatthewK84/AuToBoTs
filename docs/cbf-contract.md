# Safety filter contract

The filter sits on the setpoint request, before `decide`. It may reshape a navigation command. It may not emit a mode change, and it may not clear a `Revert`.

The filter is a pure function. Inputs are the current state, the proposed setpoint, and the constraint set. The output is a filtered setpoint plus `filter_intervened`. A true bit means the filter changed the request. It does not authorize commit.

If the quadratic program or the backup rollout fails, times out, or returns a non-finite command, the bit is irrelevant and the tick is `Revert`. The host passes that verdict to `decide`. `decide` is still the only constructor of an outbound command.

The filter crate has no MAVLink dependency and no RTLola dependency.
