# Requirements

Frozen shall statements. Each row has an id, a hazard id, and a verification method. A row with no method fails the table check. Threshold numbers live in `docs/spec-notes.md`. They are not copied here.

## Shall statements

| Id | Hazard | Shall | Method |
| --- | --- | --- | --- |
| R-H1 | H1 | The system shall revert on fence exit, including the stopping-distance margin. The margin is the host bit in `docs/spec-notes.md`. | test |
| R-H2 | H2 | The system shall revert on a stale position fix. The age limit is the `fix_stale` row in `docs/spec-notes.md`. | test |
| R-H3 | H3 | The system shall revert on a stale command link. The age limit is the `link_stale` row in `docs/spec-notes.md`. | test |
| R-H4 | H4 | The system shall revert on a stale flight-controller heartbeat. The age limit is the `heartbeat_stale` row in `docs/spec-notes.md`. | test |
| R-H5 | H5 | The system shall inhibit on a weak track inside commit range. The confidence and range limits are the `weak_track` and `inside_commit` rows in `docs/spec-notes.md`. | test |
| R-H6 | H6 | The system shall reject a non-finite numeric input and revert. | test |
| R-H7 | H7 | The system shall revert when monitor eval exceeds the deadline, whether or not eval returns, and latch revert until process restart. The deadline is `deadline_ms` in config, not a copied number. | test |
| R-H8 | H8 | The system shall latch revert on an interpreter load or accept error until process restart. | test |
| R-H9 | H9 | The system shall not emit setpoints and a mode change in the same tick. `Command` is an enum. | test |
| R-H10 | H10 | The system shall latch revert on a log write failure in flight until process restart. | test |
| R-H11 | H11 | The system shall refuse an unsigned reload and keep the running monitor. | test |
| R-H12 | H12 | The system shall refuse a spec that flips a must-hold fixture, including a learned threshold change without a bundle. | test |
| R-H13 | H13 | The system shall refuse a swap whose shadow eval exceeds the deadline. | test |
| R-H14 | H14 | The system shall not activate a learned weight file without a bundle. | test |
| R-H15 | H15 | The system shall latch revert on a fault mid-swap and stay on the old spec until a spec has passed quarantine. | test |
| R-R1 | recovery | The system shall resend the recovery command on a bounded backoff until the flight controller reports the recovery mode. It shall fall back down the order in `docs/recovery.md` on a `DENIED` or `UNSUPPORTED` ack. It shall send no attempt twice from the tick thread and the watchdog. When the order is used up unconfirmed, it shall stop sending and keep Revert latched until process restart. | test |
| R-S1 | startup | With `startup.mode = "armed"`, the system shall send no setpoint and no recovery command, from the tick thread, the watchdog, or shutdown, while the flight controller reports disarmed. It shall still evaluate and log every tick. | test |
| R-S2 | startup | The system shall command nothing before a flight-controller heartbeat and a fresh fix. An arm seen before that shall latch Revert until process restart and log `not_ready_at_arm`. | test |
| R-A1 | authority | The system shall not commit without a valid grant. A mode change is not a grant. Revert beats a valid grant. | test |

## Verification

A method is `test`, `analysis`, or `review`. A blank method is a failed row. Thresholds are not copied from `docs/spec-notes.md`.

## Review

A requirement change names the old shall, the new shall, and the tests that move with it. CI fails a requirements hunk with no test hunk. A requirements-only diff is refused.

## Derived

Switch invariants written as shalls. Each names the enforcing type and a test id. A row with no test id fails the table check.

| Id | Shall | Enforcing type | Test id |
| --- | --- | --- | --- |
| D-1 | The system shall not emit setpoints and a mode change in the same tick. | `SwitchCommand` | `revert_has_no_commit_field` |
| D-2 | The system shall not commit on inhibit. | `decide` | `inhibit_clears_commit` |
| D-3 | A missing verdict shall be revert. | `decide_input` | `missing_verdict_is_revert` |

## Port

The tracker port contract. The tracker crate meets it. The switch does not import the tracker.

| Id | Shall | Enforcing type | Test id |
| --- | --- | --- | --- |
| P-1 | Confidence shall be in 0 to 1. | `accept` | `confidence_outside_range_is_rejected` |
| P-2 | Range shall be non-negative. | `accept` | `negative_range_is_rejected` |
| P-3 | Sample time shall be monotonic. | `PortGate` | `a_step_back_in_sample_time_is_rejected` |

## Timing

Tick, deadline, and watchdog misses are shalls. The number comes from config. It is not copied here.

| Id | Shall | Source | Test id |
| --- | --- | --- | --- |
| T-1 | The tick period shall be `tick.period_ms` from config. | `config/example.toml` | `period_comes_from_config` |
| T-2 | The deadline shall be `tick.deadline_ms` from config. A late result is revert. | `config/example.toml` | `deadline_comes_from_config` |
| T-3 | The watchdog shall trip after `tick.watchdog_misses` missed ticks. | `config/example.toml` | `watchdog_comes_from_config` |
