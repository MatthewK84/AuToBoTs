# Barrier evidence

This note states what the backup filter showed and what it did not. It is not a compliance claim.

## Enclosing architecture

ASTM F3269-21 is the enclosing architecture. The filter sits on the setpoint request, before `decide`. ASTM F3269 still applies to the switch around this filter. The filter is a complex function with a recovery. It is not a replacement for the recovery.

The recovery the switch trusts is the flight-controller mode: return, loiter, or land. In the PX4 tree that is `AUTO_RTL`, `AUTO_LOITER`, and `AUTO_LAND` in `px4_custom_mode.h`, implemented by `src/modules/navigator/rtl.cpp`, `loiter.cpp`, and `land.cpp`. The companion does not fly that mode. It asks the flight controller to.

## Shown

Forward invariance of the backup set under the point-mass model, in tests.

The model is a point-mass double integrator using `v_max` and `a_max`. The backup policy is brake toward a rally, or hold when the rally is not finite. Constraints in this version are the fence polygon, the altitude floor and cap, and the speed cap.

| Claim | Test |
| --- | --- |
| A command that stays in the interior is unchanged | `interior_command_is_unchanged` |
| A command aimed through the margin band is reshaped or rejected | `command_through_the_margin_is_reshaped_or_rejected` |
| A rollout longer than the deadline yields `Timeout`, consumed as `Revert`, not the unfiltered command | `rollout_past_the_deadline_is_not_the_command` |

Those tests run on the model. They do not run on an aircraft.

## Not shown

Invariance under the real aircraft is not shown. Invariance under wind is not shown. Invariance under a wrong fence is not shown.

Also not shown: PX4 return-mode altitude, cone, and land delay (`RTL_RETURN_ALT`, `RTL_CONE_ANG`, `RTL_LAND_DELAY`); attitude dynamics; actuator lag; drag; a fence that does not match the polygon the filter was given. A camera blob is not a barrier input.

## Residual

Residual model error is explicitly unmeasured. Drag, wind, attitude dynamics, actuator lag, and fence survey error have no number in this note. The filter does not inflate the fence to cover them. That residual is not a hidden margin.

A failed, timed-out, or non-finite rollout is `Revert`. `decide` remains the only constructor of an outbound command.
