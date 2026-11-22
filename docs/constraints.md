# Constraints

Group 1–3 bounds. A task that breaks a constraint says so in the same change.

| Constraint | Bound | Notes |
| --- | --- | --- |
| Tick deadline | 5 ms | Late result discarded; tick is Revert |
| Power / thermal | Group 1–3 aircraft surplus (example 12 W) | No second high-power compute assumed |
| Shadow net | Compile-out | `rta-host` does not link a second untrusted network; the feature is gated off |
| No hot reload | Process restart | Spec change is a restart and golden-trace review |
| Fail-closed | Any fault → Revert | Late tick, interpreter error, NaN, dead heartbeat, log failure |
| Recovery mode | Config only | RTL, loiter, or land; no buried default |
| Complex function | Never reaches socket | Switch is the only constructor of an outbound command |

The shadow-net compile-out rule is a hard row. See `docs/shadow-net.md`.

## Config range checks

`period_ms`, `deadline_ms`, and `watchdog_misses` have upper bounds as well as the existing lower bounds. A zero period is an error.

## Default refusal

Missing recovery mode is an error. There is no RTL default in code.

## Example config

The example is the reviewed parameter item. A comment at the top says it is not a flight file.

## Flight config diff

A flight file is a named path with its own hash. The diff against example is an open item until a flight file exists.
