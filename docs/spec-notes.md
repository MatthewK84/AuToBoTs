# Spec notes

Thresholds live in `spec/monitor.lola` and in this file. A change edits both. The host has no copy.

| Stream | Hazard | Units | Threshold | Why |
| --- | --- | --- | --- | --- |
| fix_stale | H2 | ms | 500 | A fix older than half a second is not a commit source. |
| link_stale | H3 | ms | 300 | Command link older than the tick budget several times over. |
| heartbeat_stale | H4 | ms | 1000 | One missed flight-controller heartbeat at 1 Hz. |
| weak_track | H5 | confidence | 0.40 | Below this, a track inside commit range inhibits. |
| inside_commit | H5 | m | 150 | Commit range. Not a fence. |
| revert | H1-H4 | bool | fence or any stale stream | Revert beats inhibit. |
| inhibit | H5 | bool | revert or weak track inside commit range | Drops commit only. |

Triggers use reason codes `fence`, `fix`, `link`, `heartbeat`, `weak_track`. Not sentences.

Fixtures in `spec/fixtures/verdicts.json` name the expected verdict for each case. M7.3 replays them. The load test checks that the spec still contains every stream those fixtures depend on.

Blind spots: smoke, decoy, wrong target, an estimator that is fresh and wrong, a tracker that is confident and wrong.
