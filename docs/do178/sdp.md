# Software Development Plan

This is a repository planning document. It is not a certification claim.

## Level and standards

The software-level choice is in `docs/do178/psac.md`. It is a project choice, not an authority finding. The switch and the monitor are recommended at DO-178C Level C. The untrusted complex function is recommended at Level E. The split is the point of ASTM F3269-21.

DO-178C is the development guidance named here. DO-333 is named for the formal-methods hook on `decide` and on the RTLola replay. Objective identifiers are not cited from memory. The identifiers in hand are listed in the PSAC.

## Development

The switch is a pure decision function. It does not link an interpreter, MAVLink, or a release actuator. The monitor loads the RTLola spec once and evaluates a tick. The complex function proposes a setpoint. The switch accepts or rejects it.

Standards in use are the Rust style guide and `rustfmt`. Public items carry doc comments. A missing doc sentence that this file requires fails the planning test.

## Hazards

Development of the switch and the monitor addresses H1 through H10. Bundle and quarantine development addresses H11 through H15. A change that removes a hazard row is a development defect, not a cleanup.
