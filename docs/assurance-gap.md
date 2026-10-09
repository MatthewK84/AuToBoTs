# Assurance gap

The monitor and the recovery are not pedigreed components.

This build bounds flight and commit. It does not assure the tactical decision.

ASTM F3269 is the architecture being followed. Following it is not a compliance claim.

## Trace

Each M1.1 hazard has a row. H7 through H10 are the later rows already in `docs/hazards.md`. A missing hazard id is not covered by this table.

| Hazard | Spec stream | Test |
| --- | --- | --- |
| H1 | `fence_ok` | golden `hazard=H1` |
| H2 | `fix_stale` on `fix_age_ms` | golden `hazard=H2` |
| H3 | `link_stale` on `link_age_ms` | golden `hazard=H3` |
| H4 | `heartbeat_stale` on `fc_heartbeat_age_ms` | `h4_heartbeat_gap_reverts` |
| H5 | `weak_track` and `inside_commit` | `h5_zero_confidence_inside_commit_does_not_pass` |
| H6 | none; the host rejects the record | `h6_nan_altitude_reverts` |
| H7 | none; the host timer discards a late eval | `late_tick_discards_pass` |
| H8 | none; the host latches an interpreter error | `h8_interpreter_err_reverts` |
| H9 | none; the switch command is one enum | `revert_has_no_commit_field` |
| H10 | none; a failed log flush latches revert | `write_failure_latches_and_retries` |

Target identity, collateral, rules of engagement, tracker truth, and a fresh wrong estimate are not rows. This note does not turn them into assured decisions.
