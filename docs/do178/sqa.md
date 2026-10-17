# Software Quality Assurance Plan

This is a repository planning document. It is not a certification claim and not an authority finding.

## Level and standards

The software-level choice is in `docs/do178/psac.md`. It is a project choice, not an authority finding. Quality assurance of the switch and the monitor follows the Level C recommendation. The complex function is Level E. The split is the point of ASTM F3269-21. An applicant can raise the level. This repository does not.

DO-178C is the quality-assurance guidance named here. DO-333 is named so the Kani and replay evidence is not treated as a qualified-tool claim. `rtlola-interpreter` and Kani are not qualified tools. That gap is written here, not folded into a compliance sentence.

## Assurance

A plan that cannot point at a hazard id is incomplete. The five planning documents point at H1 through H15. A test reads them and fails if a hazard id or the project-choice sentence is missing.

## Non-claim

This repository does not implement a weapon, a fuze, or a release actuator. These documents do not fill a signature block.
