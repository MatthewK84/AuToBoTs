# Stress runs and host pauses

`scripts/stress.sh` runs the real-time tests of `rta-host` (`runtime::` and `recovery::`) 60 times on an idle host and 60 times beside 16 busy loops, and prints the failures per phase. Each failure is classified below before anything is changed. The 5 ms production tick deadline is not a test knob and is never moved to make a run pass.

## Three kinds of failure

| Kind | What it looks like | What is done |
|---|---|---|
| Product bug | Fails the same way on a quiet host, or the log shows the host doing something the documents forbid | Fixed, with a deterministic regression test |
| Test assumption too tight for a loaded host | The host did the documented, safe thing, but the test assumed timing the load took away | The test is fixed so it still checks the safety property on every run |
| Whole-host pause | Every thread stops at once; `CLOCK_MONOTONIC` jumps by seconds | Detected and reported as `host paused`, not counted as a failure |

## Whole-host pauses

The development box is a virtual machine that is snapshotted and resumed. On resume the kernel logs `random: crng reseeded due to virtual machine fork`, and `Instant` (`CLOCK_MONOTONIC`) has jumped. Any real-time test that spans the jump can fail. It is not a defect in the code and says nothing about a vehicle.

`crates/rta-host/src/host_pause.rs` (tests only) guards every runtime test. A sampler thread wakes every 5 ms and records each gap of 1 s or more between wakes. 1 s is three times the longest stall CPU starvation alone produced (320 ms, below) and two hundred times the tick deadline. When a test fails and such a gap overlaps it, the guard prints `host paused: <test> attempt N: monotonic gap ...` and runs the test again. The rerun's result stands. A failure with no gap fails as usual and is never retried. Three failures in a row, each across a gap, fail as `inconclusive, not a pass`. With `RTA_HOST_PAUSE_LOG` set, the guard also appends the line to that file, and `scripts/stress.sh` counts those runs.

## The failures from the PR #210 stress run

PR #210's stress run (60 idle, 60 under 16 busy loops on 8 CPUs) had one failed idle run and two failed loaded runs:

| Run | Tests | Kind | Evidence |
|---|---|---|---|
| idle 56 | `a_vehicle_that_never_switches_gets_the_documented_order_then_silence`, `arm_after_ready_then_a_breach_sends_one_auto_rtl_and_disarm_is_monitor_only` | Whole-host pause | The send instants jump from 31764.17 s to 31771.09 s on `CLOCK_MONOTONIC`; the kernel logged a VM-fork reseed at 31770.56 s, inside that gap. Wall time for the run was 4.5 minutes; the test binary reported 13.9 s; every other idle run took 6.9 to 7.0 s |
| load 58 | `a_hung_evaluator_reverts_within_one_deadline_and_the_fc_gets_recovery`, `a_slow_evaluator_past_the_deadline_reverts_and_latches` | Whole-host pause | 10.7 s and 11.8 s against a 3 s bound; the run reported 23.8 s against 7.1 to 7.6 s for the other 59; 146 s of wall time between the log files of runs 57 and 58; a VM-fork reseed at 32242.6 s falls inside it |
| load 1 | `arm_after_ready_then_a_breach_sends_one_auto_rtl_and_disarm_is_monitor_only` | Test assumption | No pause (7.46 s run, no reseed). At the arm tick `fix_age_ms=320`, over the 200 ms readiness limit: the busy loops starved the fake flight controller's stream and the tick thread. `rta-host` correctly latched `not_ready_at_arm` and reverted. The test assumed the arm would always land on a ready tick |

The arm test now runs the scenario up to three times. Every run is checked: nothing is commanded before the arm or while disarmed. A run whose arm landed on a stale fix must show `not_ready_at_arm` with `fix_age_ms` over 200, latched and reverted, and is then tried again. A `not_ready_at_arm` with a fresh fix fails at once. The first run whose arm lands on a ready tick must show the full ready path: one AUTO.RTL, confirmed, monitor-only after the disarm. Three runs with no ready arm fail the test.

No product bug was found in these runs. Before and after numbers are in the pull request that added this note.

## PX4 SITL under load

`scripts/sitl-px4.sh` shares the host with PX4. Two host-timing outcomes are told apart from a result (`docs/sitl.md`, "Host deadline misses"): `rta-host` missing its 5 ms deadline (it latches Revert, by design), and PX4's own simulator starving (PX4 logs `No valid data from` its SIH sensors and fails safe on its own). Either one is recorded, kept as `<scenario>.attempt1.json`, and the scenario is run once more.
