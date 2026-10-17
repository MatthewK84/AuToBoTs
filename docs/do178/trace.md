# Trace

Generated from `trace: Hn` tags in the tests. A hazard with no tag fails CI.

Verification results are the CI log and the Kani log. The CI log is the `main ci` GitHub Actions run. The Kani log is the output of `cargo kani -p rta-switch`, named in `docs/kani.md`.

| Hazard | Requirement | Evidence | Test | Result |
| --- | --- | --- | --- | --- |
| H1 | fence exit, including the stopping-distance margin, is Revert | host fence_ok | margin_band_is_not_ok | ci-log |
| H2 | stale position fix is Revert | spec stream fix_age_ms | position_resets_fix_age | ci-log |
| H3 | stale command link is Revert | spec stream link_age_ms | silent_gap_over_the_spec_limit_is_stale | ci-log |
| H4 | stale flight-controller heartbeat is Revert | spec stream fc_heartbeat_age_ms | missing_heartbeat_is_stale | ci-log |
| H5 | weak track inside commit range is Inhibit | spec stream range_m and track_conf | fake_emits_a_weak_track_inside_commit_range | ci-log |
| H6 | non-finite numeric input is rejected | host record constructor | rejects_non_finite_confidence_and_negative_age | ci-log |
| H7 | eval longer than deadline_ms is Revert | host timer | late_tick_discards_pass | ci-log |
| H8 | interpreter load or accept_event error is Revert | host latch on Err | fault_stays_revert | ci-log |
| H9 | setpoints and a mode change are not both a command | switch invariant: Command is an enum | decide_variants_are_exclusive | kani-log |
| H10 | log write failure in flight is Revert | host latch on a failed flush | write_failure_latches_and_retries | ci-log |
| H11 | unsigned reload is refused | bundle gate | unsigned_is_rejected_and_the_monitor_stays | ci-log |
| H12 | a spec that flips must-hold is refused | quarantine fixture | a_spec_that_flips_must_hold_is_refused | ci-log |
| H13 | an over-deadline eval is refused | quarantine deadline | an_over_deadline_eval_is_refused | ci-log |
| H14 | a candidate weight file does not change inference | active path | a_candidate_weight_file_does_not_change_inference | ci-log |
| H15 | a fault mid-swap stays on the old spec | host latch | a_fault_mid_swap_cannot_leave_the_command_path_on_the_new_spec | ci-log |
