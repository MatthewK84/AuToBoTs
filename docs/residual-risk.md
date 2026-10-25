# Residual risk

Hazards whose after-control probability is not zero. This repository does not accept them on anyone's behalf. None of these rows is accepted as closed.

| Hazard | Remaining control | Who would have to accept it |
| --- | --- | --- |
| Wrong-but-fresh position estimate | `fix_age_ms` and the grant window. The fence, the barrier, and the shadow net all consume the same estimate. | An authorizing official, after a measurement this repository does not have. |
| Confident wrong track | The confidence threshold in `docs/spec-notes.md`. A confident wrong track still commits if the grant is valid. | An authorizing official. |
| Grant issued on bad information | Signature, window, and fence hash. A grant is still a grant. | The person who signs the grant. |
| Model error in the barrier | The point-mass double integrator. It is not an aerodynamic model. | An authorizing official, after a flight-test residual this repository does not have. |

These are the common-mode items from `docs/common-mode.md`. The estimator-ok bit is the trip M140.3 owns. A healthy flag that is wrong stays a residual. This note does not close it.
