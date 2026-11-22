# Tests

## Unit test inventory (M50.1)

Public functions in `rta-switch` and the test that covers them.

| Function | Test name | Status |
| --- | --- | --- |
| `decide` | `pass_does_not_invent_commit`, `inhibit_clears_commit`, `revert_has_no_commit_field`, `pilot_override_suppresses_setpoints_same_tick`, `revert_beats_pilot_override` | Covered |
| `decide_input` | `pilot_override_does_not_commit`, `energy_at_or_below_rally_reverts`, `estimator_failure_reverts_to_recovery`, etc. | Covered |
| `energy_ok` | `energy_ok_is_false_for_every_nan_ordering` | Covered |

A public function with no test is a row marked missing. Missing fails the inventory job once it exists.

## Integration test inventory (M51.1)

SITL scripts and sink tests, with the hazard each covers.

| Script / test | Hazard | Notes |
| --- | --- | --- |
| `scripts/sitl-px4.sh` | H1, H4, recovery | PX4 SITL smoke |
| Runtime sink tests in `rta-host` | H7, H8, startup gate | Fake flight controller |

A script with no hazard id fails review.

## Regression suite (M52.1)

`cargo test --workspace` plus `rta-replay` on fixtures. Required before a spec change merges. CI job name: `test` (and any replay job). A red replay blocks the spec gate.

## Mutation check (M53.1)

Procedure: temporarily flip the Revert arm in `decide` to Pass, confirm the table test fails, then restore. The live match arm still maps Revert to a mode command. Mutation is not left in the tree.

## Flaky test policy (M59.1)

A SITL test that depends on wall sleep is marked timing-sensitive and is not a merge blocker. Deterministic sink tests are the blocker. No new sleep-only assertion is required in CI.
