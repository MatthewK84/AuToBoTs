# Timing

The numbers live in `config/example.toml`. This file names the keys. It does not copy the values.

- Tick period is `tick.period_ms`. It stays at the example value until M10 measures the target.
- Deadline is `tick.deadline_ms`. A result later than that deadline is discarded. The tick is `Revert` even if the interpreter then returns pass.
- Using the last verdict on timeout is rejected. A late pass is not a pass.
- Watchdog is `tick.watchdog_misses` missed published ticks. The watchdog thread issues the recovery command. It does not call the interpreter.
- Flight-controller mode-change latency is `tick.mode_change_latency_ms`. That value is an assumption until M10 measures the bench. The fence margin in M2.3 has to cover it.

The monitor must trip early enough that recovery can still recover. That is the requirement. The stopping-distance margin is how the fence meets it.
