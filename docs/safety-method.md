# Safety method

This is the project scale. It is not a finding and not a certification claim.

The scale is MIL-STD-882E. Severity is Table I: Catastrophic (1), Critical (2), Marginal (3), Negligible (4). Probability is qualitative: Frequent (A), Probable (B), Occasional (C), Remote (D), Improbable (E). Eliminated (F) is not used. Nothing here is claimed eliminated.

## Rows

Before is the hazard with no monitor. After is the same hazard with the named control. Both columns are filled. The control is a spec stream, a switch invariant, or a grant check, and it exists as a stream or a test.

| Hazard | Severity | Before | After | Control |
| --- | --- | --- | --- | --- |
| H1 | Critical (2) | Occasional (C) | Remote (D) | spec stream `fence_ok`, test `margin_band_is_not_ok` |
| H2 | Critical (2) | Occasional (C) | Remote (D) | spec stream `fix_age_ms`, test `position_resets_fix_age` |
| H3 | Critical (2) | Occasional (C) | Remote (D) | spec stream `link_age_ms`, test `silent_gap_over_the_spec_limit_is_stale` |
| H4 | Critical (2) | Occasional (C) | Remote (D) | spec stream `fc_heartbeat_age_ms`, test `missing_heartbeat_is_stale` |
| H5 | Critical (2) | Occasional (C) | Remote (D) | spec stream `track_conf`, test `fake_emits_a_weak_track_inside_commit_range` |
| H6 | Critical (2) | Probable (B) | Improbable (E) | host record constructor, test `rejects_non_finite_confidence_and_negative_age` |
| H7 | Critical (2) | Occasional (C) | Remote (D) | host timer, test `late_tick_discards_pass` |
| H8 | Critical (2) | Occasional (C) | Remote (D) | host latch, test `fault_stays_revert` |
| H9 | Critical (2) | Occasional (C) | Improbable (E) | switch invariant `Command`, test `decide_variants_are_exclusive` |
| H10 | Marginal (3) | Occasional (C) | Remote (D) | host latch, test `write_failure_latches_and_retries` |
| H11 | Critical (2) | Occasional (C) | Remote (D) | bundle gate, test `unsigned_is_rejected_and_the_monitor_stays` |
| H12 | Critical (2) | Occasional (C) | Remote (D) | quarantine fixture, test `a_spec_that_flips_must_hold_is_refused` |
| H13 | Marginal (3) | Occasional (C) | Remote (D) | quarantine deadline, test `an_over_deadline_eval_is_refused` |
| H14 | Critical (2) | Occasional (C) | Remote (D) | active path, test `a_candidate_weight_file_does_not_change_inference` |
| H15 | Critical (2) | Occasional (C) | Remote (D) | host latch, test `a_fault_mid_swap_cannot_leave_the_command_path_on_the_new_spec` |

After is not zero. Residual items stay residual. This table does not accept risk on anyone's behalf.
