# Common-mode failures

A shared input is one measurement consumed by more than one control. This note lists them. It does not claim sensor diversity the aircraft does not have.

## Shared inputs

| Input | Consumers | Control | What the control is not |
| --- | --- | --- | --- |
| Position estimate | Fence check, barrier, shadow net | `fix_age_ms` and the grant window | An independent measurement. A wrong-but-fresh estimate defeats all three. |
| Fix age | Spec `fix_stale`, host age | The age itself | A check that the fix is correct. Fresh and wrong passes it. |
| Track confidence and range | Spec `weak_track`, tracker port | The confidence threshold in `docs/spec-notes.md` | Tracker truth. A confident wrong track still commits if the grant is valid. |
| Grant | Authority check, indication | Signature, window, fence hash | A second sensor. A grant issued on bad information is still a grant. |

## Position

The fence check, the barrier, and the shadow net all consume the same position estimate. A wrong-but-fresh estimate defeats all three. The control is the fix-age stream plus the grant window. That is not an independent measurement. There is no second position source on this aircraft.

The estimator-ok bit is the trip M140.3 owns. A healthy flag that is wrong stays a residual. This note does not close it.

## What is not claimed

This aircraft does not have a second GNSS or a second IMU. No sentence here says it does.
