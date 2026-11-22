# Continuous test evidence

DoWI 8430.01 section 3.5.d asks for evidence of operational effectiveness, suitability, and survivability. This repository produces developmental evidence only.

The hook stores `cargo test`, replay, and SITL logs as named artifacts and labels them DT&E, not OT&E.

| Artifact | Label | Note |
| --- | --- | --- |
| switch-coverage | DT&E | coverage job |
| sitl-trips | DT&E | stand-in peer |
| sitl-px4 | DT&E | PX4 SITL, not hardware |

The SITL claim table is in `docs/sitl.md`. No artifact is labeled operational test.

This note is the acceptance record for M141.3.
