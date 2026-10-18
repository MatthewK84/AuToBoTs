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
| H6 | NaN or infinity in any numeric input | Host record constructor rejects the sample. `TickInput::try_new` rejects a non-finite or negative energy value, and `decide_input` reverts on one | Revert |
| H7 | Monitor eval longer than `deadline_ms` | Host timer, not the interpreter | Revert, and the late interpreter result is discarded |
| H8 | Interpreter load or `accept_event` error | Host latch on any `Err` | Revert, latched until process restart |
| H9 | Setpoints and a mode change in the same tick | Switch type: `Command` is an enum, not two independent flags | Unreachable |
| H10 | Log write failure in flight | Host latch on a failed flush | Revert, latched until process restart |
| H11 | Unsigned reload | Bundle gate `unsigned_is_rejected_and_the_monitor_stays` | swap refused |
| H12 | Fixture regression, including a learned threshold change inside the spec without a bundle | Quarantine `a_spec_that_flips_must_hold_is_refused` | swap refused |
| H13 | Swap timeout, shadow eval over the deadline | Quarantine `an_over_deadline_eval_is_refused` | swap refused |
| H14 | Learned weight file activated without a bundle | Active path `a_candidate_weight_file_does_not_change_inference` | swap refused |
| H15 | Fault mid-swap | Host latch `a_fault_mid_swap_cannot_leave_the_command_path_on_the_new_spec` | Revert, latched until a spec has passed quarantine |

A learned change to a threshold inside the spec, without a bundle that passes quarantine, is H12. It is a hazard, not a feature.

## Non-hazards

These are not rows. A later task may not smuggle them in as if this list already covered them.

Hot reload is not a non-hazard and it is not out of scope. Unsigned reload, fixture regression, swap timeout, a learned file without a bundle, and a fault mid-swap are H11 through H15.

- Target identity.
- Collateral.
- Rules of engagement, beyond the signed grant already required for commit.
- Tracker truth. Confidence is an input, not a claim that the track is the right object.
- Estimator truth beyond age and the estimator-ok bit. A fresh wrong position remains a residual.

## Later rows

M140 adds pilot override, energy to rally, estimator health, mission phase, abort of a sent commit, coast quality, protected-entity inhibit, traffic, and grant-time conversion. Those issues own their rows. They are not silently covered by H1 through H10.
