# Shadow network

A second network may propose setpoints. It is a second implementation of the complex-function trait (`rta_switch::Complex`). It is not a recovery control function in the ASTM F3269 sense, because a network is not a pedigreed simple controller.

Neither network can construct a `Command`. Both return `Option<Request>`. `decide` remains the only constructor of an outbound command.

## Selection

The primary proposal is filtered and switched. The shadow proposal is logged every tick, including ticks where it is not active.

The shadow becomes the active proposal only if the primary has not produced a finite request for a configured number of ticks, and only while the spec verdict is not `Revert`. Activation is the logged reason code `shadow_takeover`.

A `Revert` verdict drops both proposals. If both are silent, the tick is `Revert`. A non-finite request counts as silence.

Until the silence threshold is met, a silent primary with a live shadow yields no active proposal. That is not `Revert`. The shadow is still logged.

## Weight pin

Weights are a file with a recorded hash. `load_pinned` refuses a hash mismatch, a missing pin, and an empty expected hash. A load failure keeps the primary and does not activate the shadow.

## Inference budget

`shadow.inference_budget_ms` is a fraction of `tick.deadline_ms`. It starts at 2 ms. Over-budget inference is a silent primary. A silent primary is not `Pass` by itself.

## Group 1-3 constraint

The shadow exists only if the measured power and the deadline still close. If they do not, the shadow is compiled out. The deadline is not raised.

Power and deadline result, recorded before the shadow is allowed on by default:

- Named board: NVIDIA Jetson Orin Nano 8GB. Not measured. Issue 33 stays open.
- Deadline: 5 ms. Not raised. Host stand-in worst eval was a scheduler stall, not a reason to change the spec.
- Power: unmeasured. The module class is about 10 W. That is not a wattmeter reading. The 12 W aircraft surplus is a budget, not a close.
- Decision: `SHADOW_DEFAULT_ON` is false. The shadow is compiled out until an Orin run is under the deadline and a wattmeter closes the power.
