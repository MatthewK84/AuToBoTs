# Hazards

A hazard enters this file before it enters the spec or a test. No new hazard without a row. Verdicts are `Revert`, `Inhibit`, or unreachable.

`Revert` beats `Inhibit`. An inhibit row is not consulted when a revert row is true.

| Id | Hazard | Detected by | Verdict |
| --- | --- | --- | --- |
| H1 | Fence exit, including the stopping-distance margin | Host `fence_ok` or the flight-controller fence bit | Revert |
| H2 | Stale position fix | Spec stream on `fix_age_ms` | Revert |
| H3 | Stale command link | Spec stream on `link_age_ms` | Revert |
| H4 | Stale flight-controller heartbeat | Spec stream on `fc_heartbeat_age_ms` | Revert |
| H5 | Weak track inside commit range | Spec stream on `range_m` and `track_conf` | Inhibit, unless a revert row is also true |
| H6 | NaN or infinity in any numeric input | Host record constructor rejects the sample | Revert |
| H7 | Monitor eval longer than `deadline_ms` | Host timer, not the interpreter | Revert, and the late interpreter result is discarded |
| H8 | Interpreter load or `accept_event` error | Host latch on any `Err` | Revert, latched until process restart |
| H9 | Setpoints and a mode change in the same tick | Switch type: `Command` is an enum, not two independent flags | Unreachable |
| H10 | Log write failure in flight | Host latch on a failed flush | Revert, latched until process restart |

## Non-hazards

These are not rows. A later task may not smuggle them in as if this list already covered them.

- Target identity.
- Collateral.
- Rules of engagement, beyond the signed grant already required for commit.
- Tracker truth. Confidence is an input, not a claim that the track is the right object.
- Estimator truth beyond age and the estimator-ok bit. A fresh wrong position remains a residual.

## Later rows

M140 adds pilot override, energy to rally, estimator health, mission phase, abort of a sent commit, coast quality, protected-entity inhibit, traffic, and grant-time conversion. Those issues own their rows. They are not silently covered by H1 through H10.
