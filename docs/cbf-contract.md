# Safety filter contract

The filter sits on the setpoint request, before `decide`. It may reshape a navigation command. It may not emit a mode change, and it may not clear a `Revert`.

The filter is a pure function. Inputs are the current state, the proposed setpoint, and the constraint set. The output is a filtered setpoint plus `filter_intervened`. A true bit means the filter changed the request. It does not authorize commit.

If the quadratic program or the backup rollout fails, times out, or returns a non-finite command, the bit is irrelevant and the tick is `Revert`. The host passes that verdict to `decide`. `decide` is still the only constructor of an outbound command.

The filter crate has no MAVLink dependency and no RTLola dependency.

## Backup set

The backup policy is the recovery the flight controller will fly: brake toward a rally, or hold when the rally is not finite. `backup_apply` rolls that policy forward over a fixed step. The step count is bounded by the tick deadline. A rollout with `horizon_steps` greater than `deadline_steps` returns `FilterFault::Timeout`. The switch consumes that as `Revert`. The unfiltered command is not returned.

The model is a point-mass double integrator using config `v_max` and `a_max`. Constraints in this version are the fence polygon, the altitude floor and cap, and the speed cap. A camera blob is not an input.

Residual, not a hidden margin: drag, wind, attitude dynamics, actuator lag, and fence survey error are unmeasured. This filter does not inflate the fence to cover that residual.

An intervention is logged beside the verdict as `filter_intervened`.
