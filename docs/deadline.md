# Deadline review

Issue 194 reviews the 2026-10-09 host measurement. The deadline stays 5 ms. `deadline_ms` is not raised. Issue 33 is closed with a production-test hook. The Orin run is named in `docs/production-hooks.md`.

The 60.839 ms worst case came from a tight loop: 512,611,400 samples in 10 minutes at 96 percent CPU. That sample is a host scheduling stall, not a reason to change the spec. A paced run evaluates once per 50 ms tick and records eval time separately from the sleep. The deadline check uses that eval time.

The named board is still the NVIDIA Jetson Orin Nano 8GB. This host is not that board.

Paced run, 2026-10-09, one minute on the build host: 1,194 samples, worst eval 2.363 ms, none over the 5 ms deadline, CPU 0.1 percent. The watchdog still commanded `mode Rtl` when the loop was paused. That run is in `crates/rta-host/fixtures/paced-timing.txt`.

Decision: the deadline stays 5 ms. The 60.839 ms tight-loop sample is a host stall. It is not a spec change. Issue 33 remains open for an Orin Nano measurement.

## Enforcement

The deadline is preemptive since the evaluation worker (`crates/rta-host/src/eval_worker.rs`). The RTLola monitor is built once on its own thread at startup; a parse or type error still exits before the log and the socket. Each tick hands one record over a channel of capacity one and waits for the reply on a second channel of capacity one, for at most `tick.deadline_ms`.

| Case | Verdict | Latches |
| --- | --- | --- |
| Reply inside the deadline | the spec verdict | no |
| No reply inside the deadline, hung or slow | Revert, `late` | yes |
| Previous request still running | Revert, `eval_fault` | yes |
| Worker thread gone | Revert, `eval_fault` | yes |

A fault latches like an interpreter error. The worker is not asked again until restart, and a reply that arrives late is discarded by sequence number, never read as a later tick's verdict. A hung evaluator holds its own thread only. The tick thread, the receive thread, and the watchdog do not wait on it, and shutdown detaches it.

No queue grows and no thread is spawned per tick. The watchdog stays the backstop for a tick thread that stalls anywhere else.

Latching changes one consequence: a single over-deadline evaluation, including a host scheduling stall like the 60.839 ms tight-loop sample, now holds Revert for the rest of the run instead of one tick.
