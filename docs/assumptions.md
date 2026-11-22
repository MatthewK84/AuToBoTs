# Assumptions

Assumptions until a measurement replaces them. Each names the task that can retire it. None is written as a measured fact.

| Assumption | Value | Retires when | Task |
| --- | --- | --- | --- |
| Flight-controller mode-change latency | 200 ms | Bench measurement of actual mode-change time under load | M10 |
| Withdraw budget | Next tick only; command already on the wire is not cancelled | Explicit cancel path exists and is tested | M140.5 / #183 |
| Stopping-distance margin | \(v^2 / 2a\) plus latency \(\times v_{max}\) | Measured deceleration and latency on the target vehicle | M2.3 / M10 |
| Heartbeat period | 1 Hz (1000 ms healthy, 1500 ms stale limit) | Confirmed against production FC firmware | M3.2 |
| Tick period | 50 ms | Target timing measurement shows a different sustainable rate | M1.3 / M10 |
| Deadline | 5 ms | Target timing measurement of eval duration | M4.3 / M10 |

A later measurement that changes a number updates this file and the config or spec in the same change.
