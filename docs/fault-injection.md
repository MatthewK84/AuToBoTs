# Fault injection

Five injections, one test each, named by hazard id. None of them may pass.

| Test | Hazard | Injection | Required outcome |
| --- | --- | --- | --- |
| `h8_interpreter_err_reverts` | H8 | Injected interpreter `Err` on the next accept, then the latch | Revert |
| `h8_spec_missing_at_startup` | H8 | Spec path does not exist | Startup failure, before a socket |
| `h4_heartbeat_gap_reverts` | H4 | Flight-controller heartbeat age over the spec limit | Revert |
| `h6_nan_altitude_reverts` | H6 | NaN altitude | Rejected before the spec, host verdict Revert |
| `h5_zero_confidence_inside_commit_does_not_pass` | H5 | Confidence 0 inside commit range | Inhibit, never Pass |

H5 stays Inhibit because the hazard table says a weak track is not itself a revert. A revert row still wins if it is also true.
