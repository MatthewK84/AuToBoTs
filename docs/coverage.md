# Coverage

`rta-switch` is gated. `decide` and its fail-closed match arms are the target: 100 percent. An uncovered `Revert` arm fails the job. Host and tracker coverage is reported and not gated until their software level says so.

The report is a CI artifact, `target/coverage/switch.txt`. The PSAC names this target.

Coverage is not a proof. A covered line can still be wrong. The Kani harness and the golden traces are the other evidence.
