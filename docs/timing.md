# Timing

The numbers live in `config/example.toml`. This file names the keys. It does not copy the values.

- Tick period is `tick.period_ms` (shall T-1). It stays at the example value until M10 measures the target.
- Deadline is `tick.deadline_ms` (shall T-2). A result later than that deadline is discarded. The tick is `Revert` even if the interpreter then returns pass. The tick thread does not wait past the deadline for a hung evaluator, and the miss latches. Detail in `docs/deadline.md`.
- Using the last verdict on timeout is rejected. A late pass is not a pass.
- Watchdog is `tick.watchdog_misses` (shall T-3) missed published ticks. The watchdog thread issues the recovery command. It does not call the interpreter.
- Flight-controller mode-change latency is `tick.mode_change_latency_ms`. That value is an assumption until M10 measures the bench. The fence margin in M2.3 has to cover it.

The monitor must trip early enough that recovery can still recover. That is the requirement. The stopping-distance margin is how the fence meets it.

- Withdraw budget is one tick plus link age. The starting number is 100 ms after receipt. It is an assumption until M10 measures it. A withdraw that arrives mid-tick inhibits the next command. It does not retract the command already handed to the socket in that tick. That command is logged as in flight.
