# Spec notes

Thresholds live in `spec/monitor.lola` and in this file. A change edits both. The host has no copy.

| Stream | Hazard | Units | Threshold | Why |
| --- | --- | --- | --- | --- |
| fence_ok | H1 | bool | host margin, not the legal line | The host publishes the bit. The spec does not see vertices. |
| fix_age_ms | H2 | ms | input | The host ages the fix. The spec compares it. |
| link_age_ms | H3 | ms | input | Command-link age, separate from the flight-controller heartbeat. |
| fc_heartbeat_age_ms | H4 | ms | input | Heartbeat age from local receive time. |
| track_conf | H5 | 0.0 to 1.0 | input | Confidence is not tracker truth. |
| range_m | H5 | m | input | Range presented to the spec. A held-out range is not this field. |
| fix_stale | H2 | ms | 500 | A fix older than half a second is not a commit source. |
| link_stale | H3 | ms | 300 | Command link older than several ticks. |
| heartbeat_stale | H4 | ms | 1000 | One missed flight-controller heartbeat at 1 Hz. |
| weak_track | H5 | confidence | 0.40 | Below this, a track inside commit range inhibits. |
| inside_commit | H5 | m | 150 | Commit range. Not a fence. |
| revert | H1-H4 | bool | fence or any stale stream | Revert beats inhibit. |
| inhibit | H5 | bool | revert or weak track inside commit range | Drops commit only. |

Triggers use reason codes `fence`, `fix`, `link`, `heartbeat`, `weak_track`. Not sentences.

## Blind spots

These match the non-hazards in `docs/hazards.md`. They are not rows and this spec does not cover them.

- Target identity.
- Collateral.
- Rules of engagement, beyond the signed grant already required for commit.
- Tracker truth. A tracker that is confident and wrong still passes `weak_track`. Smoke and a decoy are this case.
- Estimator truth beyond age. An estimator that is fresh and wrong still passes `fix_stale`.
