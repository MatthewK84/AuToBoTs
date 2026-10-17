# Software Configuration Management Plan

This is a repository planning document. It is not a certification claim.

## Level and standards

The software-level choice is in `docs/do178/psac.md`. It is a project choice, not an authority finding. Configuration control applies to the Level C switch and monitor and to the Level E complex function alike. The split is the point of ASTM F3269-21. It does not excuse an unsigned change.

DO-178C is the configuration-management guidance named here. The Parasoft ebook names Table A-8 for the software configuration management process, including a control category by software level. DO-333 does not replace it. Objective identifiers are not cited from memory. The documents in hand are listed in the PSAC.

## Control

The repository is the configuration item. A change lands on `main` only after CI. The close job closes an issue only when the commit names it and CI passed. Issue 33 and issue 49 stay open because their board measurements are missing. That is configuration truth, not a waiver.

A spec change is a bundle. An unsigned reload is H11. A learned file without a bundle is H14.

## Hazards

H11 through H15 are configuration hazards. H1 through H10 are controlled by the same repository and the same CI gate.
