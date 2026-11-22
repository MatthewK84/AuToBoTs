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

## File descriptors

The host holds the link socket and the log file. A leak test opens and closes a short run and checks the count.

## Socket reconnect

A dropped socket latches Revert and retries the mode command. Reconnect does not clear the latch.

## Partial write

A short write is a write failure. It latches Revert. It does not retry the setpoint half.

## Tracker backpressure

The appsink is leaky. A slow detector drops frames and ages the sample. It does not block the host tick.

## Queue bound

Inbound messages are bounded. Overflow drops the oldest and ages the input. Overflow is a log field.

## Drop policy

Drop oldest, never drop the revert decision. A full queue cannot erase a pending Revert.

## Overload degrade

Over deadline, the degrade row is Revert, not a quieter Pass.

## Config schema

The TOML has a schema version. An unknown version refuses to start. Unknown keys error.

## Counters

Deadline miss, heartbeat miss, grant use, shadow takeover, filter intervention, and reload refusal are counters in the log or indication stream. An increase does not clear a Revert. The spec still owns the revert decision. Withdraw resets nothing; it only inhibits. shadow_takeover is not a health bit the spec trusts. Repeated filter intervention is a review item, not an automatic pass. A refused bundle keeps the old spec.

## Memory and allocation

A resident-set note for the host on the named board is written next to the power note. A growth across the 10-minute run is recorded, not explained away. The tick path documents where it allocates. A new allocation in decide is a review failure. decide stays allocation-free. The interpreter and the filter name their stack use as unknown until measured. Unknown is a gap row, not a zero.

## Log rotation and integrity

Log rotation is a residual until a flight file exists. The integrity hash is the spec_hash in the log header. A mismatch fails replay.

## Clock drift

Clock drift bound is a residual. The monotonic clock ignores a backward step. A wall step larger than the grant bound latches grant inhibit.

## Tick overrun

Tick overrun is counted. An overrun is Late, which is Revert. The histogram is a residual until measured on the board.

## Command and mode ack

A command ack is expected. A mode ack timeout latches Revert and retries the mode command. The setpoint is not retried half.

## Captured link replay

A captured link can be replayed through the same parser. Ages are computed from the capture timestamps, not wall time.

## Dialect mismatch

A dialect mismatch at startup refuses to start. The configured dialect and the observed dialect must match.

## Tracker sample clock

The tracker sample clock is the host monotonic clock. A sample older than the hold limit is confidence 0.

## Confidence and range

Confidence is in [0, 1]. A value outside is confidence 0. Range is non-negative. A negative range is a miss.

## Hold limit

The hold limit is the tracker_hold_ms in config. Exceeding it is confidence 0, not a last-good value.

## Detector timeout

A detector that does not answer within the deadline is confidence 0. It does not block the tick.

## Pipeline restart

A pipeline restart is bounded. A restart that exceeds the bound is confidence 0 and a log field. It does not clear a Revert.

## Frame drop

A dropped frame ages the sample. The drop count is a log field. It does not block the tick.
