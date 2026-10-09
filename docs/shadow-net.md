# Shadow network

A second network may propose setpoints. It is a second implementation of the complex-function trait (`rta_switch::Complex`). It is not a recovery control function in the ASTM F3269 sense, because a network is not a pedigreed simple controller.

Neither network can construct a `Command`. Both return `Option<Request>`. `decide` remains the only constructor of an outbound command.

## Selection

The primary proposal is filtered and switched. The shadow proposal is logged every tick, including ticks where it is not active.

The shadow becomes the active proposal only if the primary has not produced a finite request for a configured number of ticks, and only while the spec verdict is not `Revert`. Activation is the logged reason code `shadow_takeover`.

A `Revert` verdict drops both proposals. If both are silent, the tick is `Revert`. A non-finite request counts as silence.

Until the silence threshold is met, a silent primary with a live shadow yields no active proposal. That is not `Revert`. The shadow is still logged.
