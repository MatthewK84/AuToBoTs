# Known problems

Residual risks in operator language. This file links the hazard ids. It does not accept a risk or assign an owner.

| Problem | Hazard | What the operator sees |
| --- | --- | --- |
| Fresh wrong position | H residual | Fence and barrier both pass. Estimator-ok bit is the control; a wrong-but-healthy flag stays residual. |
| Missing battery sample | energy | Host publishes a large default until the flight-controller battery message is decoded. |
| Grant path refused | grant_time | Missing or negative wall-clock offset at startup means no commit. Setpoints may still pass. |

The runbook links this file. No personal name is invented as an owner.
