# AuToBoTs engineering tasks

Fail-closed runtime assurance for a Group 1–3 aircraft. The flight controller owns actuators and recovery. A Rust host owns I/O and the switch. An RTLola specification owns every predicate.

M0–M10 bound flight and commit. Control-barrier filtering, a second untrusted network, spec reload, commit authority, the video tracker, and the DO-178C evidence package are M11–M16 in docs/engineering-tasks-m11.md. Those milestones do not replace the fail-closed switch.

Each `###` heading is one GitHub issue. Do the milestones in order. M3 and M4 can run in parallel after M2. M10 can start once M5 exists.

Frozen decisions, do not relitigate in a task:

- Thresholds live in `spec/monitor.lola`. The host has no numeric threshold constants.
- Any fault maps to `Revert`: late tick, interpreter error, missing verdict, NaN input, dead heartbeat, log write failure in flight.
- A latched interpreter fault clears only on process restart.
- Recovery is a mode the flight controller already honors: RTL, Loiter, or Land. Config chooses which. Code does not.
- The complex function cannot reach the MAVLink socket. It submits a request. The switch disposes of it.
- `Revert` beats `Inhibit` beats `Pass`.


## M20–M39

These twenty milestones are part of this file. Task text is under the matching heading below. Issues #59–#78.

| Milestone | Issue | Task |
| --- | --- | --- |
| M20 Requirement baseline | #59 | Shall statements, each with a hazard and a verification method |
| M21 System safety method | #60 | Severity and probability before and after the monitor |
| M22 Flight-controller interface control | #61 | Message contract, including companion-death failsafe |
| M23 Time base | #62 | Monotonic clock; a wall-clock step must not shrink ages |
| M24 Reproducible build | #63 | Lockfile, toolchain, and runner image in the configuration index |
| M25 Static analysis | #64 | Unwrap and expect banned on the switch path |
| M26 Structural coverage | #65 | Branch coverage gated on `decide` |
| M27 Common-mode failures | #66 | Shared position estimate named, not claimed diverse |
| M28 Watchdog independence | #67 | Watchdog does not call the interpreter, filter, or tracker |
| M29 Parameter data | #68 | Config hash treated as its own configuration item |
| M30 Startup and shutdown | #69 | Config, spec, log, then socket; recovery before exit |
| M31 Degraded-mode matrix | #70 | No degraded row allows commit |
| M32 Log schema version | #71 | Unknown log version is rejected, not guessed |
| M33 Operator latency budget | #72 | Withdraw inhibits the next tick, not the command already sent |
| M34 Simulation claims | #73 | What SITL shows and what it does not |
| M35 Tracker adversarial cases | #74 | Frozen frame, empty frame, NaN confidence, two tracks |
| M36 Spec change gate | #75 | Spec edit without notes fails CI |
| M37 Release checklist | #76 | Ground release tag refuses a dirty index |
| M38 Data rights and third-party code | #77 | License list matches `cargo deny` |
| M39 Residual risk note | #78 | What is not closed, and who would have to accept it |

---

## M0 — Repository and toolchain

### M0.1 Cargo workspace skeleton

Labels: `repo`

Depends on: none. Blocks: every crate task.

Empty workspace that compiles, formats, and lints. No behavior yet. The crate split is the architecture, so it is fixed here.

Crates:

- `rta-spec` — input record, verdict enum, spec load contract. No sockets.
- `rta-switch` — pure function from verdict plus request to command. No sockets, no interpreter.
- `rta-host` — binary. Config, clock, interpreter, MAVLink, log, watchdog.
- `rta-replay` — binary. Reads a tick log, re-evaluates the spec, diffs verdicts.

Work:

1. `cargo new` is the wrong tool. Write the workspace `Cargo.toml` with `resolver = "2"` and members `crates/*`.
2. Each crate gets its own `Cargo.toml`, `src/lib.rs` or `src/main.rs`, and a one-line doc comment stating what it is forbidden to depend on. `rta-switch` must not depend on `rtlola-interpreter` or `mavlink`. `rta-spec` must not depend on either.
3. Pin the toolchain in `rust-toolchain.toml` to a stable channel with an explicit version, not floating `stable`.
4. Commit `Cargo.lock`.
5. `rustfmt.toml` with `edition = "2021"` matching the crate edition.
6. CI on push and pull request: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
7. README first screen states scope and non-scope, links to this file, and names the crate split.

Acceptance:

- [ ] `cargo test --workspace` passes on a clean checkout.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `rta-switch` Cargo.toml has no rtlola or mavlink dependency.
- [ ] README scope paragraph matches the frozen decisions above.

### M0.2 Dependency pins

Labels: `repo`

Depends on: M0.1.

Pins so a later crate task does not drift the stack.

Work:

1. Add `rtlola-interpreter` and `rtlola-frontend` to `rta-host` only. Record the exact version and the commit or crates.io checksum in `docs/dependencies.md`.
2. Add `mavlink` to `rta-host` only. Decide the dialect in this task: `ardupilotmega` if the bench aircraft is ArduPilot, `common` if it is PX4. Write the decision and the reason in `docs/dependencies.md`. Do not depend on both.
3. Add `kani-verifier` as an optional dependency of `rta-switch`, feature `verify`, off by default. `cargo test` must not require the Kani solver.
4. Add `cargo-deny` config: allow MIT, Apache-2.0, BSD-3-Clause; deny copyleft; advisory check in CI as a separate job so a network blip does not hide a compile failure.
5. If a crate will not build on the pinned toolchain, stop and record the blocker in the issue. Do not swap in a different rules engine inside this task.

Acceptance:

- [ ] `docs/dependencies.md` names versions, dialect, and why.
- [ ] Default `cargo test --workspace` does not build Kani.
- [ ] `cargo deny check` job exists and is not required for the compile job.

### M0.3 Configuration contract

Labels: `repo`

Depends on: M0.1.

One TOML file is the only runtime configuration. Missing or invalid config is a startup failure before any socket opens.

Fields:

- `link.endpoint` — `udp:host:port` or `serial:path:baud`.
- `link.system_id`, `link.component_id`.
- `tick.period_ms` — start at 50.
- `tick.deadline_ms` — start at 5.
- `tick.watchdog_misses` — start at 3.
- `recovery.mode` — enum `rtl | loiter | land`.
- `spec.path` — path to `monitor.lola`.
- `log.path`, `log.flush_every_n`.
- `fence` — either `source = "fc"` or a polygon plus vertical cap plus `v_max` and `a_max` for the stopping-distance margin.

Work:

1. Parse with `serde`. Unknown fields error.
2. `period_ms` must be greater than `deadline_ms`. `watchdog_misses` must be at least 1.
3. Example `config/example.toml` checked in.
4. Unit test: missing file, bad enum, deadline greater than period, valid example.

Acceptance:

- [ ] Invalid config exits non-zero and does not open a socket. Cover this with a test that the parse function returns `Err` before any I/O type is constructed.
- [ ] Example config round-trips.

---

## M1 — Safety boundary

### M1.1 Hazard list

Labels: `requirements`

Depends on: none. Blocks: M1.2, M3.2, M8.

Write `docs/hazards.md`. Each hazard is a row the spec and the tests must trace to. No new hazards without a row.

Rows:

- H1 fence exit. Detected by host fence bit or FC fence bit. Verdict `Revert`.
- H2 stale fix. `fix_age_ms` over spec limit. Verdict `Revert`.
- H3 stale command link. `link_age_ms` over spec limit. Verdict `Revert`.
- H4 stale flight-controller heartbeat. `fc_heartbeat_age_ms` over spec limit. Verdict `Revert`.
- H5 weak track inside commit range. `range_m` under commit limit and `track_conf` under confidence limit. Verdict `Inhibit` unless a revert hazard is also true.
- H6 NaN or infinity in any numeric input. Host rejects the record. Verdict `Revert`.
- H7 monitor timeout. Eval longer than `deadline_ms`. Verdict `Revert` even if the interpreter then returns pass.
- H8 interpreter fault. Any `Err` from load or `accept_event`. Latched `Revert`.
- H9 setpoints and mode-change in the same tick. Switch invariant. Must be unreachable.
- H10 log write failure in flight. Latched `Revert`.

Non-hazards, written in the same file so they are not smuggled in later: target identity, collateral, rules of engagement, tracker truth, estimator truth beyond age.

Acceptance:

- [ ] Every row names the detecting stream or switch invariant and the verdict.
- [ ] Non-hazards section exists.

### M1.2 Verdict and recovery map

Labels: `requirements`

Depends on: M1.1. Blocks: M5.

Define the enum in `rta-spec` and the recovery map in `docs/verdicts.md`.

- `Pass` — forward the latest setpoint request if one exists. Commit is allowed.
- `Inhibit` — drop commit/release. Navigation setpoints may still forward, and only if no revert hazard is true.
- `Revert` — emit no setpoint. Command `recovery.mode`.

Priority is structural: the host computes revert first. Inhibit is not consulted when revert is true.

Recovery mode is config. The switch receives an already-chosen mode. It does not contain the string `"RTL"` as a default buried in a match arm that ignores config.

Acceptance:

- [ ] Enum and docs agree.
- [ ] A test table in `rta-spec` maps each hazard id to a verdict.
- [ ] No code path treats inhibit as a license to keep a commit command.

### M1.3 Timing budget

Labels: `requirements`

Depends on: M0.3, M1.2.

Write `docs/timing.md`.

- Tick 50 ms until a target measurement says otherwise.
- Deadline 5 ms. A late result is discarded and the tick is `Revert`.
- Watchdog: 3 missed published ticks forces a recovery command from a thread that does not call the interpreter.
- Assumed flight-controller mode-change latency is written as an assumption, start at 200 ms, and the fence margin has to cover it. The number is an assumption, labeled as one, until M10 measures the bench.

The monitor must trip early enough that recovery can still recover. That sentence is the requirement. The stopping-distance margin in M2.3 is how it is met for the fence.

Acceptance:

- [ ] Numbers live in the example config and are referenced, not copied, by the timing doc.
- [ ] "Use last verdict on timeout" is explicitly rejected.

---

## M2 — Input contract

### M2.1 Input record

Labels: `spec`

Depends on: M1.1. Blocks: M3, M4, M6.

`rta-spec::InputRecord`:

- `alt_m: f64`
- `fence_ok: bool`
- `fix_age_ms: f64`
- `track_conf: f64` in 0.0..=1.0
- `range_m: f64`
- `link_age_ms: f64`
- `fc_heartbeat_age_ms: f64`

Work:

1. Private fields, constructor `try_new`.
2. Reject NaN and infinity on every float. Reject confidence outside 0.0..=1.0. Reject negative ages and negative range.
3. Serde representation version-tagged. Field order stable. Adding a field is a version bump and a replay-header bump.
4. `Display` or a log helper that prints every field in that order.

Acceptance:

- [ ] Table test for NaN altitude, infinite range, confidence 1.1, negative age.
- [ ] Successful constructor round-trips through serde.

### M2.2 Stale and missing policy

Labels: `spec`

Depends on: M2.1. Blocks: M6.3.

The host ages samples. The spec decides they are bad. Do not invert that.

- Missing tracker sample: hold last confidence and range, set an age. When tracker age exceeds the written hold limit, publish `track_conf = 0` and keep the last range only as a logged extra, not as a spec input that could still pass the commit check. Start hold limit 200 ms, in config, documented as a host policy not a spec threshold.
- Missing position: do not invent a position. `fix_age_ms` grows from the last accept time until the spec trips.
- First tick with no sample yet: ages are larger than any reasonable spec limit, so the first verdict is `Revert` until fresh data arrives. That is correct. Do not special-case startup into `Pass`.

Acceptance:

- [ ] Unit test: no tracker sample ever, confidence presented to the spec is 0.
- [ ] Unit test: position gap grows `fix_age_ms` and does not reuse a stale coordinate as fresh.

### M2.3 Fence ownership

Labels: `spec`

Depends on: M2.1, M1.3.

Decide in this task and write it in `docs/fence.md`. Default decision: host-evaluated polygon from config, because the flight-controller fence trips after breach and this monitor must trip early.

Work:

1. Polygon inclusion in the host. Output is the bool `fence_ok`. The spec never sees raw vertices.
2. Vertical cap in the same check.
3. Margin equals stopping distance `v^2 / 2a` using config `v_max` and `a_max`, plus the assumed mode-change latency times `v_max`. The legal fence line is not the trip line.
4. `fence_ok = false` when the polygon is missing or degenerate. Fail closed.

Acceptance:

- [ ] Point inside the margin band is `fence_ok = false`.
- [ ] Point in the interior is `fence_ok = true`.
- [ ] Empty polygon is `fence_ok = false`.

---

## M3 — Specification

### M3.1 Specification layout

Labels: `spec`

Depends on: M0.2, M2.1.

`spec/monitor.lola` is the only predicate source.

Work:

1. CI step parses the spec with `rtlola-frontend` and fails the build on syntax error.
2. Host requires output streams named `revert` and `inhibit`. Names live in a const list in `rta-spec`. A spec missing either fails at load, before arming, before the socket opens.
3. No hot reload. A spec change is a process restart and a golden-trace review.

Acceptance:

- [ ] Broken spec fails CI.
- [ ] Spec with no `revert` stream fails load in a unit test.

### M3.2 First monitor specification

Labels: `spec`

Depends on: M1.1, M3.1.

Streams, names fixed so the host and the fixtures can depend on them:

- `fix_stale` from `fix_age_ms`
- `link_stale` from `link_age_ms`
- `heartbeat_stale` from `fc_heartbeat_age_ms`
- `weak_track` from `track_conf`
- `inside_commit` from `range_m`
- `revert` = not `fence_ok` or `fix_stale` or `link_stale` or `heartbeat_stale`
- `inhibit` = `revert` or (`inside_commit` and `weak_track`)

Starting thresholds, written in the spec and in `docs/spec-notes.md`, open to change only by editing both: fix 500 ms, link 300 ms, heartbeat 1000 ms, confidence 0.40, commit range 150 m.

Triggers carry stable reason codes: `fence`, `fix`, `link`, `heartbeat`, `weak_track`. Not sentences.

Acceptance:

- [ ] Fixture inputs from M7.3 produce the verdicts named in those fixtures.
- [ ] Host source contains none of the threshold literals.

### M3.3 Spec review notes

Labels: `spec`

Depends on: M3.2.

`docs/spec-notes.md` has one short note per stream: hazard id, units, threshold, why that number. Blind spots listed in the same file: smoke, decoy, wrong target, estimator that is fresh and wrong, tracker that is confident and wrong.

Acceptance:

- [ ] Every stream in `monitor.lola` has a note.
- [ ] Blind spots section exists and matches the non-hazards in M1.1.

---

## M4 — Interpreter host

### M4.1 Load and build

Labels: `host`

Depends on: M3.1, M0.3.

Work:

1. Read `spec.path` from config. Missing file is a startup `Err`.
2. Build the RTLola monitor once.
3. Do not watch the file. Document that in the runbook stub.

Acceptance:

- [ ] Missing spec does not open a socket. Test at the load function.
- [ ] Second tick uses the same monitor value, not a rebuild.

### M4.2 Tick evaluation

Labels: `host`

Depends on: M4.1, M2.1, M1.2.

Work:

1. Push one `InputRecord` per tick through `accept_event` with a monotonic timestamp in the interpreter's time unit. Convert at the boundary and test the conversion.
2. Read `revert` and `inhibit`. Map to the enum. Absent or unknown stream is `Revert`.
3. Capture trigger reason codes into the tick result.
4. Any interpreter `Err` latches a fault and returns `Revert`.

Acceptance:

- [ ] Mapping test: revert true → `Revert`, inhibit true and revert false → `Inhibit`, both false → `Pass`.
- [ ] `Err` test latches and stays `Revert` on the next call even if the next event would pass.

### M4.3 Deadline

Labels: `host`

Depends on: M4.2, M1.3.

Work:

1. Time the evaluation.
2. If duration exceeds `deadline_ms`, the tick result is `Revert` and the interpreter output is discarded.
3. Duration is part of the tick result so the log can record it.

Acceptance:

- [ ] A fake evaluator that sleeps past the deadline yields `Revert`.
- [ ] A fast pass still yields `Pass`.

---

## M5 — Switch

### M5.1 Command ownership

Labels: `switch`

Depends on: M1.2. Blocks: M6.

The complex function implements a trait in `rta-switch`:

```text
fn request(&mut self) -> SetpointRequest
```

`SetpointRequest` is either `None` or navigation setpoints plus an optional commit flag. The trait object or generic is held by the host. The only type that can construct an outbound command is `switch::decide`.

In-tree stub returns `None`.

Acceptance:

- [ ] `rta-switch` has no MAVLink dependency.
- [ ] A test proves the stub's request never appears in a `Revert` command.

### M5.2 Verdict dispatch

Labels: `switch`

Depends on: M5.1.

`decide(verdict, request, reported_mode, recovery_mode) -> Command`.

- `Pass`: forward setpoints, include commit only if the request asked for it.
- `Inhibit`: forward navigation setpoints, force commit off.
- `Revert`: no setpoints. Mode command equal to `recovery_mode`, unless `reported_mode` already equals it, in which case the command is empty.
- Same-tick exclusion is a type invariant: `Command` is an enum, not a struct with two independent bools. Variants are `Setpoints`, `Mode`, `Idle`.

Acceptance:

- [ ] Table test covers all three verdicts, with and without a request, with reported mode equal and not equal to recovery.
- [ ] Impossible to construct a command that is both setpoints and a mode change.

### M5.3 Fail-closed paths

Labels: `switch`

Depends on: M5.2, M4.2.

Host rules, tested at the host boundary with a fake switch:

- No verdict this tick → call `decide` with `Revert`.
- Late tick → `Revert`, discard the late verdict.
- Latched interpreter fault → `Revert` until process restart. No clear method on the public host API.
- Heartbeat older than the spec limit is already a spec revert; also treat a missing heartbeat sample as stale.
- Socket write failure latches `Revert`, logs the error, process stays up, keeps trying the mode command.

Acceptance:

- [ ] Each path has a test with the expected command variant.
- [ ] Public API has no `clear_fault`.

### M5.4 Watchdog

Labels: `switch`

Depends on: M5.3, M1.3.

A thread that does not call the interpreter checks that the eval loop has published a tick. After `watchdog_misses` missed periods it issues the recovery mode command itself.

Work:

1. Publish a tick counter or timestamp from the eval loop.
2. Watchdog reads it. It does not read the interpreter.
3. Watchdog command path is the same mode-command function the switch uses, so idempotence still holds.

Acceptance:

- [ ] Pausing the eval loop in a test causes a mode command within the miss window.
- [ ] Watchdog source does not reference the interpreter type.

---

## M6 — MAVLink adapter

### M6.1 Link

Labels: `io`

Depends on: M0.2, M0.3, M5.1.

Work:

1. UDP and serial from `link.endpoint`.
2. Heartbeat out at 1 Hz with the configured system and component id, so the flight controller sees the companion.
3. Inbound heartbeat age from local receive time. Do not trust only the remote timestamp.

Acceptance:

- [ ] Against a local MAVLink sink, a heartbeat is observed within 1.5 s of connect.
- [ ] A paused peer grows `fc_heartbeat_age_ms`.

### M6.2 State inputs

Labels: `io`

Depends on: M6.1, M2.1.

Position and altitude messages drive `alt_m` and `fix_age_ms`. Which message ids depends on the dialect frozen in M0.2; write them in `docs/mavlink.md`.

Link age is the age of the command/telemetry socket used for the companion's own uplink, separate from the flight-controller heartbeat age. Both fields exist because they fail differently.

System status is logged in v1 and is not a predicate. Do not add a hidden battery or estimator check in the adapter.

Acceptance:

- [ ] A replayed position message updates altitude and resets fix age.
- [ ] No system-status field is read by the spec.

### M6.3 Tracker input port

Labels: `io`

Depends on: M2.2.

Trait in `rta-host`: `confidence`, `range_m`, `sample_time`. Fake tracker in-tree. No GStreamer, no video decode, no pixel format. The port is the boundary.

Acceptance:

- [ ] Fake can be scripted to emit a weak track inside commit range.
- [ ] Crate does not depend on a video library.

### M6.4 Command out

Labels: `io`

Depends on: M5.2, M6.1.

Work:

1. Map `Command::Mode` to the dialect's mode-change message for the configured recovery mode.
2. Map `Command::Setpoints` to the offboard or guided setpoint message, and send nothing when the variant is `Mode` or `Idle` during revert.
3. Read the flight controller's reported mode and pass it into `decide` so the mode command is idempotent.

Acceptance:

- [ ] Sink test: `Revert` produces a mode message and no setpoint message.
- [ ] Second tick with reported mode already recovered produces no second mode message.

---

## M7 — Log and replay

### M7.1 Tick log

Labels: `replay`

Depends on: M2.1, M4.2.

One record per tick: inputs, verdict, reason codes, command variant actually emitted, eval duration, spec version or file hash.

Append-only. Versioned header. Flush every `log.flush_every_n` ticks.

Open failure at startup is fatal, before the socket. Write failure in flight latches `Revert`.

Acceptance:

- [ ] Record schema documented in `docs/log-format.md`.
- [ ] Write-error injection latches revert.
- [ ] Startup with an unwritable path exits non-zero.

### M7.2 Replay tool

Labels: `replay`

Depends on: M7.1, M3.2.

`rta-replay` reads a log and re-evaluates `monitor.lola`. Non-zero exit on any verdict mismatch. Mismatch prints tick number, expected, actual, and the input record.

Acceptance:

- [ ] A hand-edited verdict in a fixture fails the tool.
- [ ] An unmodified golden log exits zero.

### M7.3 Golden traces

Labels: `replay`

Depends on: M7.2, M3.2.

Fixtures, checked in:

- pass: interior fence, fresh fix, fresh link, fresh heartbeat, confidence 0.9, range 400
- inhibit only: same, but confidence 0.2 and range 100
- revert on fence
- revert on stale fix
- revert on stale link
- revert on stale heartbeat
- NaN altitude rejected before the spec, host verdict revert

CI runs replay against the fixtures. A spec edit that changes a fixture verdict fails CI until the fixture is intentionally updated in the same change.

Acceptance:

- [ ] Each fixture names its hazard id.
- [ ] CI job runs `rta-replay` on the fixture directory.

---

## M8 — Verification

### M8.1 Switch invariant tests

Labels: `verify`

Depends on: M5.2, M5.3.

Table tests for every verdict and every fail-closed input. A complex-function request never appears on the command value during `Revert`. `Inhibit` cannot carry a commit flag.

Acceptance:

- [ ] Tests live in `rta-switch` and do not need a socket.
- [ ] Commit-during-inhibit is a compile failure or a failing test, not a comment.

### M8.2 Kani harness

Labels: `verify`

Depends on: M5.2, M0.2.

Harness over `decide` only.

Proofs:

- Missing-verdict caller convention is not in Kani; the host tests cover that. Kani covers: for all verdicts and requests and modes, a `Mode` variant has no setpoint payload, and a `Setpoints` variant has no mode payload.
- `Revert` never yields `Setpoints`.
- `Inhibit` never yields a commit flag.

Kani job is separate from default CI.

Acceptance:

- [ ] `cargo kani` on `rta-switch` is documented.
- [ ] Default CI does not require the solver.

### M8.3 Fault injection

Labels: `verify`

Depends on: M4.2, M5.3, M7.1.

Tests: interpreter `Err`, spec file missing at startup, heartbeat gap, NaN altitude, zero-confidence track inside commit range. Each asserts `Revert` or startup failure, never `Pass`.

Acceptance:

- [ ] Five tests, one per injection, named by hazard id.

---

## M9 — Simulation

### M9.1 SITL harness

Labels: `sim`

Depends on: M6.

Pick one stack and record it in `docs/sitl.md`. Default: PX4 SITL. Script brings it up, host connects with the example config, heartbeats, and a nominal hover logs as `Pass` once fences and ages are inside limits.

Acceptance:

- [ ] Script is checked in.
- [ ] Ports in the script match `config/example.toml`.

### M9.2 Scripted trips

Labels: `sim`

Depends on: M9.1, M7.1.

- Fence breach produces a recovery-mode report from the flight controller and a `Revert` line in the tick log.
- Stopped heartbeat produces `Revert` in the log.
- Scripted weak track inside commit range produces `Inhibit` and no commit command.

Assertions read the tick log. A human watching a ground station is not the test.

Acceptance:

- [ ] Three scripted cases, each with a log assertion.

### M9.3 Timing note

Labels: `sim`

Depends on: M9.1, M4.3.

Record the SITL eval-duration distribution in the test artifact. Write in the log header and in `docs/sitl.md` that SITL timing is not target timing.

Acceptance:

- [ ] The disclaimer is in the doc and the log header format.

---

## M10 — Target and assurance notes

### M10.1 Target measurement

Labels: `target`

Depends on: M5.4, M4.3.

Name one companion-class board in `docs/target.md`. Run 10 minutes. Record worst-case eval duration against the deadline. Record CPU and an order-of-magnitude power note. Pause the eval loop and show the watchdog commands recovery.

Acceptance:

- [ ] Numbers are in the doc, dated, with the board name.
- [ ] If worst case exceeds the deadline, the issue stays open and the deadline or the spec gets a follow-up. Do not quietly raise the deadline in code.

### M10.2 Runbook

Labels: `docs`

Depends on: M5.3, M7.1, M3.1.

`docs/runbook.md`:

- What the operator sees on `Inhibit` versus `Revert`.
- How to pull the tick log.
- Restart is the only clear for a latched interpreter fault.
- A spec edit requires a golden-trace review in the same change.

Acceptance:

- [ ] Runbook matches the public API. No `clear_fault` step.

### M10.3 Assurance gap note

Labels: `docs`

Depends on: M1.1, M3.3, M8.1.

`docs/assurance-gap.md`:

- Trace table from hazard id to spec stream to test name.
- The monitor and the recovery are not pedigreed components.
- This build bounds flight and commit. It does not assure the tactical decision.
- ASTM F3269 is the architecture being followed. Following it is not a compliance claim.

Acceptance:

- [ ] Trace table has a row per hazard in M1.1.
- [ ] The non-claim sentences are present and unqualified.

---

---

These milestones were out of scope for M0–M10. They are in scope now because the governing documents are public. They do not replace the fail-closed switch. A later milestone that faults still maps to `Revert`.

Public documents named here:

- ASTM F3269-21, Standard Practice for Methods to Safely Bound Behavior of Aircraft Systems Containing Complex Functions Using Run-Time Assurance.
- RTCA DO-178C, Software Considerations in Airborne Systems and Equipment Certification, and DO-333, Formal Methods Supplement to DO-178C and DO-278A.
- DoD Directive 3000.09, Autonomy in Weapon Systems, 25 January 2023. Cleared for public release. Policy is that autonomous and semi-autonomous weapon systems are designed so commanders and operators can exercise appropriate human judgment over the use of force, and that a system unable to complete an engagement inside the commanded constraints terminates the engagement or obtains additional operator input.
- Control-barrier and backup-controller safety filters as published in the open literature (Ames et al., and the backup-set formulations flown on small quadrotors and on VISTA).

This file does not claim compliance with any of those documents. It lists the work required to produce the evidence they describe.

Frozen additions:

- The recovery the switch trusts is still the flight-controller mode. A second network is an untrusted complex function, never the recovery.
- Hot reload cannot activate an unsigned or unparsed bundle. A bad reload latches `Revert` on the last accepted spec, or stays on that spec if it is still loaded.
- Commit authority is a conjunction the switch can reject. It is not an arming circuit, a fuze, or a release mechanism. Those are outside this repository.
- Video leaves the tracker crate as `confidence`, `range_m`, and `sample_time`. The switch never sees a frame.

---

## M11 — Control-barrier safety filter

### M11.1 Filter contract

Labels: `cbf`

Depends on: M5.2, M2.3.

A safety filter sits on the setpoint request, before `decide`. It may reshape a navigation command. It may not emit a mode change, and it may not clear a `Revert`.

Write `docs/cbf-contract.md`. The filter is a pure function: current state, proposed setpoint, constraint set, returns a filtered setpoint plus a bit `filter_intervened`. If the quadratic program or backup rollout fails, times out, or returns a non-finite command, the bit is irrelevant and the tick is `Revert`.

Acceptance:

- [ ] Filter crate `rta-filter` has no MAVLink and no RTLola dependency.
- [ ] Timeout or non-finite output is a `Revert` input to the switch, covered by a test.
- [ ] `decide` is still the only constructor of an outbound command.

### M11.2 Backup-controller barrier

Labels: `cbf`

Depends on: M11.1, M1.3.

Implement the backup-set form, not a hand-solved barrier for the whole flight envelope. The backup policy is the same recovery the flight controller will fly: brake toward a rally, or hold. The filter rolls that policy forward over the deadline horizon and rejects a command that leaves the recoverable set.

Constraints in v1: fence polygon already computed in M2.3, altitude floor and cap, speed cap. No perception obstacle in v1. A camera blob is not a barrier input until M15 says the track is fresh.

Work:

1. Discrete rollout, fixed step, step count bounded by the tick deadline.
2. Model is a point-mass double integrator using config `v_max` and `a_max`. Model mismatch is a documented residual, not a hidden margin.
3. Intervention logged beside the verdict.

Acceptance:

- [ ] A command aimed through the margin band is reshaped or rejected.
- [ ] A command that stays in the interior is unchanged.
- [ ] Rollout longer than the deadline yields `Revert`, not the unfiltered command.

### M11.3 Barrier evidence note

Labels: `cbf`

Depends on: M11.2.

`docs/cbf-evidence.md` states what was shown and what was not. Shown: forward invariance of the backup set under the point-mass model, in tests. Not shown: invariance under the real aircraft, wind, or a wrong fence. ASTM F3269 still applies to the switch around this filter. The filter is a complex function with a recovery, not a replacement for the recovery.

Acceptance:

- [ ] The note names ASTM F3269-21 as the enclosing architecture.
- [ ] Residual model error is quantified or explicitly unmeasured.

---

## M12 — Second network as untrusted backup

### M12.1 Shadow controller trait

Labels: `host`

Depends on: M5.1.

A second network may propose setpoints. It is a second implementation of the complex-function trait. It is not a recovery control function in the F3269 sense, because a network is not a pedigreed simple controller.

Write `docs/shadow-net.md`. Selection policy: primary proposal is filtered and switched; the shadow proposal is logged every tick; the shadow becomes the active proposal only if the primary has not produced a finite request for a configured number of ticks, and only while the spec verdict is not `Revert`. If both are silent, `Revert`.

Acceptance:

- [ ] Neither network can construct a `Command`.
- [ ] A `Revert` verdict drops both proposals.
- [ ] Shadow activation is a logged reason code `shadow_takeover`.

### M12.2 Weight pin and budget

Labels: `host`

Depends on: M12.1, M1.3.

Weights are a file with a recorded hash. Load failure keeps the primary and does not activate the shadow. Inference budget is a fraction of `deadline_ms`, start at 2 ms, in config. Over-budget inference is a silent primary, not a pass.

Group 1–3 constraint is written in the same doc: the shadow exists only if the measured power and the deadline still close. If they do not, the task ends with the shadow compiled out, not with a raised deadline.

Acceptance:

- [ ] Hash mismatch refuses the file.
- [ ] Over-budget inference does not yield `Pass` by itself.
- [ ] Power and deadline result recorded before the shadow is allowed on by default.

---

## M13 — On-aircraft learning and spec hot reload

### M13.1 Bundle format

Labels: `spec`

Depends on: M3.1, M7.3.

A reload bundle is a directory: `monitor.lola`, optional weight file, a manifest with the previous spec hash, the new spec hash, and a detached signature. Unsigned bundles are rejected. The parser from M3.1 must accept the new spec before anything is swapped.

Write `docs/reload-bundle.md`.

Acceptance:

- [ ] Unsigned, unparsable, and hash-mismatch bundles are rejected in tests.
- [ ] Rejection leaves the running monitor in place.

### M13.2 Quarantine and swap

Labels: `host`

Depends on: M13.1, M4.1, M7.3.

Swap is not immediate. The new spec is built beside the old one and run on the last N logged ticks. If any replayed verdict disagrees with a fixture marked `must_hold`, the swap is refused. If the shadow eval exceeds the deadline, the swap is refused.

On a successful swap the host logs the old hash, the new hash, and the tick number. On any fault during the swap the latched state is `Revert` until the process is on a spec that has already passed quarantine. There is still no public `clear_fault`.

On-aircraft learning, if present, may write a candidate weight file only. It may not point the active path at that file. Activation is a bundle swap.

Acceptance:

- [ ] A spec that flips a `must_hold` fixture is refused.
- [ ] A candidate weight file on disk does not change inference until a bundle swap says so.
- [ ] Fault mid-swap cannot leave the command path on the new spec.

### M13.3 Learning hazard row

Labels: `requirements`

Depends on: M1.1, M13.2.

Add hazards to `docs/hazards.md`: unsigned reload, fixture regression, swap timeout, learned file activated without a bundle. Each maps to "swap refused" or `Revert`. Learning that changes a threshold inside the spec without a bundle is a hazard, not a feature.

Acceptance:

- [ ] New rows trace to tests in M13.2.
- [ ] M1.1 non-hazard list no longer says hot reload is out of scope.

---

## M14 — Commit authority beyond the inhibit conjunction

### M14.1 Authority token

Labels: `authority`

Depends on: M5.2, M1.2.

Commit is allowed only if all of the following are true in the same tick: verdict is `Pass`, filter did not reject, a fresh operator grant is on hand, the grant's geographic and time window contains the aircraft, and the grant has not been revoked. This is the software authority. It does not close a circuit, arm a device, or command a release mechanism.

The grant is a signed record: issuer, not-before, not-after, fence hash, max range, nonce. Verification failure is `Inhibit`, not a crash. Absence of a grant is `Inhibit`.

DoDD 3000.09 is the policy reference. The design has to leave a path for an operator to withhold or withdraw judgment. A grant is that path. A system that cannot meet the grant window terminates the commit and does not continue it.

Acceptance:

- [ ] No grant, expired grant, wrong fence hash, and revoked nonce are `Inhibit` tests.
- [ ] `Revert` still beats a valid grant.
- [ ] The commit flag cannot be set from the tracker crate or the filter crate.

### M14.2 Operator path

Labels: `authority`

Depends on: M14.1, M6.1.

The grant arrives on the existing link as a message the adapter parses into the token type. The adapter does not invent a grant from a mode switch. Withdraw is a message that invalidates the nonce; the next tick is `Inhibit` for commit even if the aircraft is inside the window.

The human-machine indication is a logged state the operator can read: `commit_armed`, `commit_inhibited` with a reason code, `reverted`. Reason codes are the trigger names plus `no_grant`, `grant_expired`, `grant_revoked`, `fence_hash`.

Acceptance:

- [ ] Withdraw mid-window inhibits the following tick.
- [ ] A mode change on the flight controller is not parsed as a grant.
- [ ] Reason codes are stable strings, covered by a table test.

### M14.3 Authority trace

Labels: `authority`

Depends on: M14.2, M10.3.

`docs/authority.md` traces each conjunct to a test and to the DoDD 3000.09 sentences it is meant to support: appropriate human judgment, engagement inside commanded constraints, terminate or obtain additional input if those constraints will not be met. State in the same file that this repository does not implement a weapon, a fuze, or a release actuator.

Acceptance:

- [ ] Trace table has a row per conjunct.
- [ ] The non-claim sentence is present.

---

## M15 — Video, tracker, GStreamer

### M15.1 Frame ingress

Labels: `tracker`

Depends on: M6.3.

Crate `rta-tracker`. Ingress is a GStreamer pipeline described in config: a `udpsrc` or `v4l2src`, a decode element, and an `appsink`. The sink pulls the latest buffer. A stalled pad older than the tracker hold limit yields `track_conf = 0` through the existing port. The host still does not decode.

Pipeline string lives in config. A pipeline that fails to start is a startup failure of the tracker process, and the host sees a missing tracker sample, which M2.2 already maps to confidence 0.

Acceptance:

- [ ] Recorded fixture stream produces buffers in a test.
- [ ] Killed source grows tracker age and collapses confidence.
- [ ] `rta-switch` still has no video dependency.

### M15.2 Track to port

Labels: `tracker`

Depends on: M15.1, M2.2.

A detector behind the sink emits one track: confidence, range estimate, sample time. Range may be a stub from bbox height and a config focal length; the stub is labeled a stub. Output crosses into the host only through the M6.3 trait.

No path from a frame to `decide`. A test builds the tracker crate and the switch crate in one workspace and asserts the switch package does not link a GStreamer library.

Acceptance:

- [ ] Port fields match `InputRecord` expectations.
- [ ] Stale detector output uses the M2.2 hold policy rather than a second policy.

### M15.3 Pipeline timing

Labels: `tracker`

Depends on: M15.2, M1.3.

Measure decode-plus-detect latency on the target board from M10.1. If it exceeds the tracker hold limit, the hold limit does not grow to hide it. The pipeline is simplified or the detector is replaced, and the measurement is recorded in `docs/tracker-timing.md`.

Acceptance:

- [ ] Dated measurement with the pipeline string.
- [ ] Hold limit unchanged unless M2.2 is explicitly revised in the same change.

---

## M16 — DO-178C evidence package

### M16.1 Planning data

Labels: `assurance`

Depends on: M1.1, M10.3.

Write the planning set as repository documents, not as a certification claim. PSAC, SDP, SVP, SCMP, SQA plan, in `docs/do178/`. Software level is chosen in the PSAC and justified. Start the recommendation at Level C for the switch and the monitor, and Level D or E for the untrusted complex function, and record that the split is the point of ASTM F3269. The applicant, if any, can raise the level. This repository does not.

Name DO-178C and DO-333. DO-333 is the hook for the Kani results on `decide` and for the RTLola replay argument. Do not cite an objective table from memory; quote the objective identifiers from the document in hand when the plans are written.

Acceptance:

- [ ] Five planning documents exist and point at hazard ids.
- [ ] Software-level choice is explicit and labeled a project choice, not an authority finding.

### M16.2 Trace and verification data

Labels: `assurance`

Depends on: M16.1, M8.1, M7.3.

Bidirectional trace: hazard to requirement to spec stream or switch invariant to test to result. Generated from tags in the tests so a missing tag fails CI. Verification results are the CI log plus the Kani log, stored as artifacts named in the SVP.

Tool qualification is a note, not a claim. `rtlola-interpreter` and Kani are not qualified tools until a qualification kit exists. The note says which objectives therefore still need review or an independent test.

Acceptance:

- [ ] Trace generator fails CI on an untraced hazard.
- [ ] Tool-qualification gap is written down.

### M16.3 Authorizing-official packet skeleton

Labels: `assurance`

Depends on: M16.2, M14.3, M11.3.

`docs/do178/ao-packet.md` is the index an authorizing official would be handed: planning data, trace, known gaps, residual hazards, the authority model, the statement that recovery is the flight controller, and the statement that the complex function is not pedigreed. The packet does not contain a signature block filled in by this repository.

Acceptance:

- [ ] Index links resolve inside the repo.
- [ ] Gaps from M11.3, M12.2, M13, and M16.2 are listed, not folded into a compliance sentence.

---


## M17 — Tool qualification criteria

### M17.1 Criteria for the unqualified tools

Labels: `assurance`

Depends on: M16.2, M8.2, M4.2.

DO-178C does not let an unqualified tool close an objective by itself. `rtlola-interpreter`, Kani, and the trace generator are unqualified until a kit exists. Write `docs/do178/tool-qualification.md` naming each tool, the objective it is asked to support, and the review or independent test that covers that objective while the tool is unqualified.

Acceptance:

- [ ] One row per tool used to produce verification evidence.
- [ ] No sentence that calls Kani or the interpreter a qualified tool.

### M17.2 Qualification gap tests

Labels: `assurance`

Depends on: M17.1, M7.3.

For each objective the unqualified tool touches, name the independent check. Replay fixtures cover the spec. Switch table tests cover `decide` even if Kani is not run. A CI job fails if a verification result is cited in the trace and the independent check is missing.

Acceptance:

- [ ] CI fails on a cited result with no independent check.
- [ ] The gap list in M16.3 includes this file.

---

## M18 — Software configuration index

### M18.1 Index contents

Labels: `assurance`

Depends on: M16.1, M13.1.

DO-178C planning data points at a configuration index. Write `docs/do178/sci.md` listing the spec hash, the switch crate revision, the config example hash, the tool versions from `docs/dependencies.md`, and the known gaps. A reload bundle from M13 is a new index row, not an edit to an old row.

Acceptance:

- [ ] Index names hashes, not floating branch tips.
- [ ] A bundle swap procedure says the index updates in the same change as the swap.

### M18.2 Index check

Labels: `assurance`

Depends on: M18.1.

A small check compares the recorded spec hash to `spec/monitor.lola` and fails CI on drift. Generated files are named in the index or excluded with a reason.

Acceptance:

- [ ] Tampering with the spec without updating the index fails the check.
- [ ] The authorizing-official packet links the index.

---

## M19 — Human-machine indication

### M19.1 Indication contract

Labels: `authority`

Depends on: M14.2, M5.2.

DoDD 3000.09 requires the interface to be understandable to a trained operator: which actions the operator must perform, which actions the system will perform. Write `docs/hmi.md`. States are `commit_armed`, `commit_inhibited`, `reverted`, plus the reason code from M14.2. The indication is a MAVLink named-value or statustext the adapter already owns. No second link.

Acceptance:

- [ ] Every reason code in M14.2 appears in the indication table.
- [ ] A `Revert` indication cannot be overwritten by a grant message in the same tick.

### M19.2 Indication tests

Labels: `authority`

Depends on: M19.1, M6.4.

Sink test: inhibit, revert, and grant-withdraw each produce the matching indication and no other. The indication is logged in the tick record.

Acceptance:

- [ ] Three sink cases, asserted from the log.
- [ ] M14.3 trace gains a row for the indication.

---


## M20 — Requirement baseline

### M20.1 Freeze the shall statements

Labels: `requirements`

Depends on: M1.1, M14.1.

Write `docs/requirements.md`. Each shall statement has an id, a hazard id, and a verification method: test, analysis, or review. No requirement without a verification method. Threshold numbers are referenced from the spec notes, not copied.

Acceptance:

- [ ] Every hazard in M1.1 and M13.3 has at least one shall.
- [ ] A requirement with no verification method fails a table check.

---

## M21 — System safety method

### M21.1 Severity and probability

Labels: `requirements`

Depends on: M1.1.

Write `docs/safety-method.md` using the public MIL-STD-882 categories as the scale, labeled as the project scale rather than a finding. Each hazard gets a severity and a qualitative probability before and after the monitor. The after column has to name the control: spec stream, switch invariant, or grant check.

Acceptance:

- [ ] Every hazard row has both columns filled.
- [ ] A control named here exists as a stream or a test.

---

## M22 — Flight-controller interface control

### M22.1 Mode and setpoint contract

Labels: `io`

Depends on: M6.4, M0.2.

Write `docs/icd-fc.md`. Message ids, rates, units, and the mode enum for the dialect frozen in M0.2. State what the flight controller does on a missed setpoint heartbeat, because that is the fail-closed path if the companion dies.

Acceptance:

- [ ] ICD names the messages the adapter sends and the ones it reads.
- [ ] Companion-death behavior is the flight controller's own failsafe, not a hope.

---

## M23 — Time base

### M23.1 Monotonic tick clock

Labels: `host`

Depends on: M4.2, M1.3.

Ages are computed from a monotonic clock, not wall time. A backward wall-clock step must not shrink `fix_age_ms` or extend a grant window. Write the choice in `docs/time.md`.

Acceptance:

- [ ] Test: wall clock jumps back, ages do not decrease.
- [ ] Grant expiry uses the same clock as the tick log.

---

## M24 — Reproducible build

### M24.1 Lock and toolchain record

Labels: `repo`

Depends on: M0.2, M18.1.

CI builds from `Cargo.lock` and `rust-toolchain.toml` only. Document the container or the runner image digest in `docs/do178/sci.md`. A floating runner image is a gap, written down if it cannot be pinned yet.

Acceptance:

- [ ] Two CI runs of the same commit produce the same lockfile hash.
- [ ] The index names the toolchain version.

---

## M25 — Static analysis

### M25.1 Clippy and an extra lint set

Labels: `verify`

Depends on: M0.1, M5.2.

Clippy is already denied warnings. Add a documented extra set for the switch crate: unwrap forbidden, expect forbidden outside tests, indexing forbidden on the command path. Record what the lints do not catch.

Acceptance:

- [ ] A deliberate unwrap in `rta-switch` fails CI.
- [ ] The gap note says lints are not a proof.

---

## M26 — Structural coverage

### M26.1 Coverage of the switch

Labels: `verify`

Depends on: M8.1, M16.1.

Measure line and branch coverage of `rta-switch` in CI. The PSAC names the target. Start at 100 percent of `decide` and the fail-closed match arms. Uncovered arms fail the job. Host and tracker coverage is reported and not gated until their software level says so.

Acceptance:

- [ ] CI artifact contains the coverage report.
- [ ] An untested `Revert` arm fails the job.

---

## M27 — Common-mode failures

### M27.1 Shared-sensor note

Labels: `requirements`

Depends on: M2.2, M11.2, M12.1.

Write `docs/common-mode.md`. The fence check, the barrier, and the shadow net all consume the same position estimate. A wrong-but-fresh estimate defeats all three. The control is the fix-age stream plus the grant window, and the note says that is not an independent measurement.

Acceptance:

- [ ] Each shared input lists the consumers.
- [ ] No sentence claims sensor diversity the aircraft does not have.

---

## M28 — Watchdog independence

### M28.1 Separate execution path

Labels: `switch`

Depends on: M5.4, M27.1.

The watchdog thread does not call the interpreter, the filter, or the tracker. Document the shared resources it still has: the socket and the clock. A test kills the eval thread and asserts a mode command from the watchdog path.

Acceptance:

- [ ] Source review note lists shared resources.
- [ ] Kill test passes without the interpreter running.

---

## M29 — Parameter data

### M29.1 Config as a parameter item

Labels: `assurance`

Depends on: M0.3, M18.1.

DO-178C treats parameter data as its own item. Hash `config/example.toml` into the configuration index. A flight config that differs from the example is a named file with its own hash, not an untracked edit. Unknown keys still fail closed.

Acceptance:

- [ ] Index has a row for the example config hash.
- [ ] A changed config without an index update fails M18.2.

---

## M30 — Startup and shutdown

### M30.1 Order of operations

Labels: `host`

Depends on: M4.1, M6.1, M7.1.

Write `docs/startup.md`. Order: config, spec load, log open, then socket. Shutdown: stop setpoints, send recovery mode, flush the log, then exit. A failed step does not skip ahead to the socket.

Acceptance:

- [ ] Test that a spec load error does not construct the link.
- [ ] Shutdown test asserts the mode command before the log flush completes.

---

## M31 — Degraded-mode matrix

### M31.1 What still flies

Labels: `requirements`

Depends on: M1.2, M12.1, M15.2.

Write `docs/degraded.md`. Rows are lost tracker, lost grant, shadow only, filter timeout, stale link. Columns are navigation allowed, commit allowed, recovery commanded. Commit is never allowed on a degraded row.

Acceptance:

- [ ] Matrix matches the verdict tests.
- [ ] No degraded row has commit allowed.

---

## M32 — Log schema version

### M32.1 Header and reader

Labels: `replay`

Depends on: M7.1, M7.2.

The log header carries a schema version. `rta-replay` rejects an unknown version instead of guessing fields. Adding an input field is a version bump, already required by M2.1, and this task enforces it at the reader.

Acceptance:

- [ ] A fixture with the wrong version exits non-zero.
- [ ] The current version is named in `docs/log-format.md`.

---

## M33 — Operator latency budget

### M33.1 Withdraw to inhibit

Labels: `authority`

Depends on: M14.2, M1.3.

Write the budget from withdraw message to commit inhibited: one tick plus link age, starting number 100 ms after receipt, labeled an assumption until M10 measures it. A withdraw that arrives mid-tick inhibits the next command, never the one already handed to the socket in that tick. The already-sent command is logged as in flight.

Acceptance:

- [ ] Test: withdraw during eval inhibits the following tick.
- [ ] Budget number lives in the timing doc.

---

## M34 — Simulation claims

### M34.1 What SITL shows

Labels: `sim`

Depends on: M9.2, M9.3.

`docs/sitl.md` gains a claims table. Shown: mode change on fence breach, inhibit on weak track, revert on heartbeat loss, in simulation. Not shown: aerodynamic truth, latency on the target, tracker performance. A test name may not be cited outside its column.

Acceptance:

- [ ] Each M9 script is in the shown column.
- [ ] Target timing is in the not-shown column.

---

## M35 — Tracker adversarial cases

### M35.1 Bad frames

Labels: `tracker`

Depends on: M15.2, M2.2.

Fixtures: frozen frame, empty frame, confidence NaN from the detector, two tracks when the port allows one. The port emits one sample or a miss. A miss uses the M2.2 hold policy. The detector does not pick a track inside the switch.

Acceptance:

- [ ] Four fixtures, each with an expected port output.
- [ ] Switch tests still take a port sample, not a frame.

---

## M36 — Spec change gate

### M36.1 Review before swap

Labels: `spec`

Depends on: M13.2, M3.3, M7.3.

A spec change in git or in a bundle requires the golden fixtures and `docs/spec-notes.md` in the same change. CI fails if `monitor.lola` changes and the notes file does not. The on-aircraft swap already replays `must_hold`. This gate is the ground copy of that rule.

Acceptance:

- [ ] CI test: spec hunk without a notes hunk fails.
- [ ] The reload bundle carries the notes file hash.

---

## M37 — Release checklist

### M37.1 Ground release

Labels: `docs`

Depends on: M18.1, M16.3, M10.1.

`docs/release.md` is the ordered list for a ground release: tests green, coverage artifact present, index hashes match, target measurement dated, gaps copied into the packet, spec notes current. A release tag points at that commit. No in-flight learning bundle is part of a ground release.

Acceptance:

- [ ] Checklist items map to files or CI jobs.
- [ ] Tag procedure refuses a dirty index.

---

## M38 — Data rights and third-party code

### M38.1 Notices

Labels: `repo`

Depends on: M0.2.

`docs/third-party.md` lists rtlola, mavlink, and GStreamer bindings with their licenses and the crate that depends on them. `cargo deny` remains the check. A new copyleft dependency fails CI rather than getting a waiver inside a feature task.

Acceptance:

- [ ] Each direct dependency is in the notice file.
- [ ] Deny config matches the notice.

---

## M39 — Residual risk note

### M39.1 What remains accepted

Labels: `assurance`

Depends on: M21.1, M16.3, M27.1.

`docs/residual-risk.md` lists hazards whose after-control probability is not zero: wrong-but-fresh estimate, confident wrong track, grant issued on bad information, model error in the barrier. Each row names the remaining control and the person who would have to accept it. This repository does not accept it on their behalf.

Acceptance:

- [ ] Every common-mode item from M27 appears here.
- [ ] No row says the risk is closed.

---


## M40–M139

One hundred milestones added to this file. Each heading below is the task. Issues follow the heading id.

## M40 — Requirement review gate

### M40.1 Requirement review gate

Labels: `requirements`

Depends on: M20.1.

A requirement change names the old shall, the new shall, and the tests that move with it. CI fails a requirements hunk with no test hunk.

Acceptance:

- [ ] A requirements-only diff fails the check.
- [ ] The review note is in docs/requirements.md.

## M41 — Derived requirements

### M41.1 Derived requirements

Labels: `requirements`

Depends on: M20.1, M5.2.

Derived requirements are the switch invariants written as shalls: no setpoint with a mode change, no commit on inhibit, missing verdict is revert.

Acceptance:

- [ ] Each derived shall names the enforcing type.
- [ ] A derived shall with no test id fails the table check.

## M42 — Tracker port requirements

### M42.1 Tracker port requirements

Labels: `io`

Depends on: M6.3, M20.1.

The port contract is a shall: confidence in 0 to 1, range non-negative, sample time monotonic. The tracker crate meets it. The switch does not import the tracker.

Acceptance:

- [ ] Port shalls are in the requirement file.
- [ ] A switch dependency on the tracker crate fails CI.

## M43 — Timing requirement trace

### M43.1 Timing requirement trace

Labels: `requirements`

Depends on: M1.3, M20.1.

Tick, deadline, and watchdog misses are shalls, sourced from config, not copied as a second number.

Acceptance:

- [ ] Timing doc links the shall ids.
- [ ] A hardcoded deadline outside config fails review.

## M44 — Safety change control

### M44.1 Safety change control

Labels: `requirements`

Depends on: M21.1, M36.1.

A hazard-row edit requires a spec-notes edit in the same change. The check is the same shape as the spec gate.

Acceptance:

- [ ] Hazard hunk without a notes hunk fails CI.
- [ ] The rule is written in docs/safety-method.md.

## M45 — Assumption log

### M45.1 Assumption log

Labels: `docs`

Depends on: M1.3, M33.1.

docs/assumptions.md lists mode-change latency, withdraw budget, and stopping distance as assumptions until a measurement replaces them.

Acceptance:

- [ ] Each assumption names the task that can retire it.
- [ ] No assumption is written as a measured fact.

## M46 — Constraint log

### M46.1 Constraint log

Labels: `docs`

Depends on: M12.2, M1.3.

docs/constraints.md lists the Group 1-3 bounds: deadline, power note, no second trusted net. A task that breaks a constraint says so in the same change.

Acceptance:

- [ ] The shadow-net compile-out rule is a row.
- [ ] The file is linked from the README.

## M47 — Open item log

### M47.1 Open item log

Labels: `docs`

Depends on: M16.3.

docs/open-items.md is the list of gaps the packet already names. An item closes only when a test or a measurement lands.

Acceptance:

- [ ] Each M16 gap is a row.
- [ ] A closed row names a commit.

## M48 — Waiver log

### M48.1 Waiver log

Labels: `assurance`

Depends on: M17.1.

A waived check names the check, the reason, and the expiry. A waiver with no expiry fails review. CI does not grow a silent allow list.

Acceptance:

- [ ] docs/waivers.md exists.
- [ ] cargo deny has no unexplained allow.

## M49 — Deviation record

### M49.1 Deviation record

Labels: `assurance`

Depends on: M16.1.

A deviation from a DO-178C objective names the objective, the alternative evidence, and the gap that remains. It is not a compliance sentence.

Acceptance:

- [ ] docs/do178/deviations.md exists.
- [ ] The packet index links it.

## M50 — Unit test inventory

### M50.1 Unit test inventory

Labels: `verify`

Depends on: M8.1.

docs/tests.md lists every switch table row and the test name. A public fn in rta-switch with no test is a row marked missing, and missing fails CI once the inventory job exists.

Acceptance:

- [ ] Inventory file exists.
- [ ] decide is covered by name.

## M51 — Integration test inventory

### M51.1 Integration test inventory

Labels: `verify`

Depends on: M9.2, M50.1.

Integration tests are the SITL scripts and the sink tests. The inventory says which hazard each one covers.

Acceptance:

- [ ] Each M9 script is a row.
- [ ] A script with no hazard id fails review.

## M52 — Regression suite

### M52.1 Regression suite

Labels: `verify`

Depends on: M7.3, M50.1.

The regression job is cargo test plus rta-replay on fixtures. It is the required check before a spec change merges.

Acceptance:

- [ ] CI job name is written in docs/tests.md.
- [ ] A red replay blocks the spec gate.

## M53 — Mutation check on the switch

### M53.1 Mutation check on the switch

Labels: `verify`

Depends on: M8.1.

A documented mutation: flip Revert to Pass in a copy of decide and show the table test fails. The mutation is not left in the tree.

Acceptance:

- [ ] The procedure is in docs/tests.md.
- [ ] The live match arm still maps Revert to a mode command.

## M54 — Property test on ages

### M54.1 Property test on ages

Labels: `verify`

Depends on: M2.2, M23.1.

Property test: ages are non-decreasing across a wall-clock step back, and a missing sample does not reset an age to zero.

Acceptance:

- [ ] Two properties run under cargo test.
- [ ] A forced age decrease fails the test.

## M55 — Fuzz the config parser

### M55.1 Fuzz the config parser

Labels: `verify`

Depends on: M0.3.

A cargo-fuzz target or a byte-slice property test feeds the parser. A panic is a failure. Unknown keys still error.

Acceptance:

- [ ] Target or proptest is in the repo.
- [ ] The panic-on-garbage case is a fixture.

## M56 — Fuzz the log reader

### M56.1 Fuzz the log reader

Labels: `verify`

Depends on: M7.2, M32.1.

Truncated and unknown-version logs exit non-zero and do not panic. A short property test covers both.

Acceptance:

- [ ] Reader tests exist.
- [ ] A truncated golden log is a fixture.

## M57 — Golden vector review

### M57.1 Golden vector review

Labels: `verify`

Depends on: M7.3.

Each fixture names the hazard and the expected verdict in the file, not only in the test. A review checklist is docs/golden-review.md.

Acceptance:

- [ ] Checklist exists.
- [ ] Every fixture path is listed.

## M58 — Verdict oracle

### M58.1 Verdict oracle

Labels: `verify`

Depends on: M1.2, M4.2.

One function in rta-spec is the oracle: inputs to expected verdict for the frozen thresholds. Replay and unit tests both call it. A second copy of the mapping is a defect.

Acceptance:

- [ ] Oracle is the only mapping in tests.
- [ ] Host mapping test uses it.

## M59 — Flaky test policy

### M59.1 Flaky test policy

Labels: `verify`

Depends on: M9.1.

A SITL test that depends on wall sleep is marked timing-sensitive and is not a merge blocker. Deterministic sink tests are the blocker.

Acceptance:

- [ ] Policy is in docs/tests.md.
- [ ] No new sleep-only assertion is required in CI.

## M60 — CI job map

### M60.1 CI job map

Labels: `repo`

Depends on: M0.1.

docs/ci.md names fmt, clippy, test, replay, deny, and the optional Kani job, and which ones block a merge.

Acceptance:

- [ ] Map matches the workflow files.
- [ ] Kani is listed as non-blocking.

## M61 — Required checks note

### M61.1 Required checks note

Labels: `repo`

Depends on: M60.1.

docs/ci.md says which checks should be required on main. The repository setting is recorded as a gap if this task cannot change it.

Acceptance:

- [ ] The intended required set is written.
- [ ] A gap is explicit if branch protection is untouched.

## M62 — Artifact retention

### M62.1 Artifact retention

Labels: `repo`

Depends on: M26.1, M10.1.

Coverage and target-measurement artifacts have a retention note. A release points at the artifact name, not a disappeared log.

Acceptance:

- [ ] docs/ci.md has the retention row.
- [ ] M37 checklist links it.

## M63 — Build provenance

### M63.1 Build provenance

Labels: `assurance`

Depends on: M24.1.

The configuration index gains the commit SHA and the CI run URL for the tagged build. A tag with no run URL is a gap row.

Acceptance:

- [ ] Index fields are named.
- [ ] Example row is in docs/do178/sci.md.

## M64 — Dependency update policy

### M64.1 Dependency update policy

Labels: `repo`

Depends on: M0.2.

A dependency bump is its own change, with the deny output and a note if the interpreter version moves. Spec behavior is re-replayed when the interpreter moves.

Acceptance:

- [ ] Policy is in docs/dependencies.md.
- [ ] Interpreter bump names a replay run.

## M65 — Advisory response

### M65.1 Advisory response

Labels: `repo`

Depends on: M0.2, M48.1.

A cargo deny advisory fails the advisory job or gets a waiver row with an expiry. No silent ignore.

Acceptance:

- [ ] Job behavior is written.
- [ ] Waiver file is the only escape.

## M66 — Fork and patch policy

### M66.1 Fork and patch policy

Labels: `repo`

Depends on: M38.1.

A patched crate is vendored with a reason, an upstream link, and a removal condition. No silent fork.

Acceptance:

- [ ] docs/third-party.md has the rule.
- [ ] No vendored crate exists without a row.

## M67 — Secret scanning note

### M67.1 Secret scanning note

Labels: `repo`

Depends on: M0.3.

Config examples contain no live endpoints that are secrets. docs/ci.md says secret scanning should be on, and records the gap if it is not.

Acceptance:

- [ ] Example config has no credential.
- [ ] The note exists.

## M68 — Branch protection note

### M68.1 Branch protection note

Labels: `repo`

Depends on: M61.1.

docs/ci.md records whether main rejects a direct push. If it does not, that is a gap, not a feature.

Acceptance:

- [ ] The current state is written.
- [ ] The intended state is written beside it.

## M69 — Release branch rule

### M69.1 Release branch rule

Labels: `repo`

Depends on: M37.1.

A release tag is cut from main at a green commit. A tag from a dirty tree is refused by the checklist. Hot-reload bundles are not release artifacts.

Acceptance:

- [ ] Rule is in docs/release.md.
- [ ] Bundle files are excluded.

## M70 — Log rotation

### M70.1 Log rotation

Labels: `replay`

Depends on: M7.1.

The log rolls at a config byte limit. A roll failure latches Revert, same as a write failure. The header is repeated on the new file.

Acceptance:

- [ ] Config key is named.
- [ ] Roll-failure test latches Revert.

## M71 — Log integrity hash

### M71.1 Log integrity hash

Labels: `replay`

Depends on: M7.1, M32.1.

Each flush writes a hash of the chunk. Replay reports a mismatch and exits non-zero. A mismatch does not repair the file.

Acceptance:

- [ ] Hash field is in the format doc.
- [ ] Tamper fixture fails replay.

## M72 — Clock drift bound

### M72.1 Clock drift bound

Labels: `host`

Depends on: M23.1.

The monotonic clock is compared to a second read across the tick. A backward step inside the process latches Revert.

Acceptance:

- [ ] Test injects a backward monotonic read.
- [ ] The latch has no clear method.

## M73 — Tick overrun histogram

### M73.1 Tick overrun histogram

Labels: `host`

Depends on: M4.3, M7.1.

The log records eval duration already. A summary counter of over-deadline ticks is in the trailer. The target note reads it.

Acceptance:

- [ ] Trailer field is documented.
- [ ] A late fake tick increments it.

## M74 — Deadline miss counter

### M74.1 Deadline miss counter

Labels: `host`

Depends on: M73.1 if present else M4.3.

The counter is exposed in the indication stream as a number, not a pass. An increase does not clear a Revert.

Acceptance:

- [ ] Indication name is in docs/hmi.md.
- [ ] Test covers one miss.

## M75 — Heartbeat miss counter

### M75.1 Heartbeat miss counter

Labels: `io`

Depends on: M6.1.

Missed flight-controller heartbeats increment a counter in the log. The spec still owns the revert decision.

Acceptance:

- [ ] Counter is a log field.
- [ ] Spec threshold is unchanged.

## M76 — Grant use counter

### M76.1 Grant use counter

Labels: `authority`

Depends on: M14.1.

Each tick a grant allows commit, a counter increments. Withdraw resets nothing; it only inhibits. The counter is a log field.

Acceptance:

- [ ] Field is documented.
- [ ] Withdraw test does not zero the counter by itself.

## M77 — Shadow takeover counter

### M77.1 Shadow takeover counter

Labels: `host`

Depends on: M12.1.

shadow_takeover increments a counter. The counter is not a health bit the spec trusts.

Acceptance:

- [ ] Log field exists in the format doc.
- [ ] A takeover fixture increments it.

## M78 — Filter intervention counter

### M78.1 Filter intervention counter

Labels: `cbf`

Depends on: M11.2.

filter_intervened increments a counter. Repeated intervention is a review item in the runbook, not an automatic pass.

Acceptance:

- [ ] Runbook sentence exists.
- [ ] Counter is in the tick log.

## M79 — Reload refusal counter

### M79.1 Reload refusal counter

Labels: `spec`

Depends on: M13.2.

A refused bundle increments a counter and keeps the old spec. The counter is logged.

Acceptance:

- [ ] Refusal fixture increments it.
- [ ] Active spec hash is unchanged.

## M80 — Memory budget

### M80.1 Memory budget

Labels: `host`

Depends on: M12.2, M10.1.

A resident-set note for the host on the named board is written next to the power note. A growth across the 10-minute run is recorded, not explained away.

Acceptance:

- [ ] docs/target.md has the row.
- [ ] The run length matches M10.

## M81 — Hot-path allocation

### M81.1 Hot-path allocation

Labels: `host`

Depends on: M4.2.

The tick path documents where it allocates. A new allocation in decide is a review failure. decide stays allocation-free.

Acceptance:

- [ ] Note is in the switch crate docs.
- [ ] Review rule is in docs/constraints.md.

## M82 — Stack bound note

### M82.1 Stack bound note

Labels: `host`

Depends on: M81.1.

The interpreter and the filter name their stack use as unknown until measured. Unknown is a gap row, not a zero.

Acceptance:

- [ ] Gap row exists.
- [ ] No invented byte count.

## M83 — File descriptor bound

### M83.1 File descriptor bound

Labels: `io`

Depends on: M6.1, M7.1.

The host holds the link socket and the log file. A leak test opens and closes a short run and checks the count.

Acceptance:

- [ ] Test exists or the gap is written.
- [ ] Expected fds are listed.

## M84 — Socket reconnect

### M84.1 Socket reconnect

Labels: `io`

Depends on: M6.1, M5.3.

A dropped socket latches Revert and retries the mode command. Reconnect does not clear the latch.

Acceptance:

- [ ] Retry is documented.
- [ ] Clear-on-reconnect test fails if someone adds it.

## M85 — Partial write handling

### M85.1 Partial write handling

Labels: `io`

Depends on: M6.4.

A short write is a write failure. It latches Revert. It does not retry the setpoint half.

Acceptance:

- [ ] Test uses a short-write sink.
- [ ] No setpoint follows the short write.

## M86 — Tracker backpressure

### M86.1 Tracker backpressure

Labels: `tracker`

Depends on: M15.1.

The appsink is leaky. A slow detector drops frames and ages the sample. It does not block the host tick.

Acceptance:

- [ ] Pipeline note says leaky.
- [ ] Host tick test does not wait on a frame.

## M87 — Queue bound

### M87.1 Queue bound

Labels: `host`

Depends on: M86.1.

Inbound messages are bounded. Overflow drops the oldest and ages the input. Overflow is a log field.

Acceptance:

- [ ] Bound is config.
- [ ] Overflow fixture ages the fix.

## M88 — Drop policy

### M88.1 Drop policy

Labels: `host`

Depends on: M87.1.

Drop oldest, never drop the revert decision. A full queue cannot erase a pending Revert.

Acceptance:

- [ ] Policy is in docs/startup.md.
- [ ] Test fills the queue and still reverts.

## M89 — Overload degrade

### M89.1 Overload degrade

Labels: `host`

Depends on: M31.1, M4.3.

Over deadline, the degrade row is Revert, not a quieter Pass. The matrix already says so. This task adds the overload row by name.

Acceptance:

- [ ] Matrix has an overload row.
- [ ] Commit is not allowed on that row.

## M90 — Config schema version

### M90.1 Config schema version

Labels: `repo`

Depends on: M0.3.

The TOML has a schema version. An unknown version refuses to start.

Acceptance:

- [ ] Parser test covers a future version.
- [ ] Example config carries the version.

## M91 — Unknown field policy

### M91.1 Unknown field policy

Labels: `repo`

Depends on: M0.3.

Unknown keys error. A test adds one and expects Err.

Acceptance:

- [ ] Test exists.
- [ ] Deny-unknown is the serde setting.

## M92 — Config range checks

### M92.1 Config range checks

Labels: `repo`

Depends on: M0.3, M1.3.

period_ms, deadline_ms, and watchdog_misses have upper bounds as well as the existing lower bounds. A zero period is an error.

Acceptance:

- [ ] Bounds are in the parser.
- [ ] Zero and huge values fail.

## M93 — Default refusal

### M93.1 Default refusal

Labels: `repo`

Depends on: M0.3.

Missing recovery mode is an error. There is no RTL default in code.

Acceptance:

- [ ] Test omits the key.
- [ ] Switch still receives the mode from config.

## M94 — Example config review

### M94.1 Example config review

Labels: `repo`

Depends on: M0.3, M29.1.

The example is the reviewed parameter item. A comment at the top says it is not a flight file.

Acceptance:

- [ ] Comment exists.
- [ ] Index hashes this file.

## M95 — Flight config diff

### M95.1 Flight config diff

Labels: `repo`

Depends on: M94.1.

A flight file is a named path with its own hash. The diff against example is an open item until a flight file exists.

Acceptance:

- [ ] Rule is in the index doc.
- [ ] No flight file is invented.

## M96 — Fence polygon validation

### M96.1 Fence polygon validation

Labels: `spec`

Depends on: M2.3.

A polygon with fewer than three points, or a self-intersection the checker can see, is fence_ok false.

Acceptance:

- [ ] Two bad polygons are fixtures.
- [ ] Empty polygon remains fail closed.

## M97 — Rally point validation

### M97.1 Rally point validation

Labels: `spec`

Depends on: M2.3, M96.1.

The rally point is inside the trip line, not on it. An outside rally refuses startup.

Acceptance:

- [ ] Parser check exists.
- [ ] Startup failure does not open a socket.

## M98 — Recovery mode validation

### M98.1 Recovery mode validation

Labels: `repo`

Depends on: M1.2.

The enum is rtl, loiter, land. Any other string fails config load.

Acceptance:

- [ ] Three bad strings are tests.
- [ ] The switch has no fourth mode.

## M99 — System id collision check

### M99.1 System id collision check

Labels: `io`

Depends on: M6.1.

If the first heartbeat seen uses this component's system id and component id, startup fails. The companion does not talk over the flight controller.

Acceptance:

- [ ] Test feeds a colliding heartbeat.
- [ ] Failure happens before setpoints.

## M100 — Spec stream inventory

### M100.1 Spec stream inventory

Labels: `spec`

Depends on: M3.2.

docs/spec-notes.md lists every output stream. A stream in the file and not in the notes fails the spec gate.

Acceptance:

- [ ] Inventory matches monitor.lola.
- [ ] Host-required names are marked.

## M101 — Unused stream check

### M101.1 Unused stream check

Labels: `spec`

Depends on: M100.1 if present else M3.1.

An output stream the host does not read is allowed only if the notes say why. Silent streams fail review.

Acceptance:

- [ ] Rule is in the spec notes.
- [ ] revert and inhibit are marked read.

## M102 — Trigger code stability

### M102.1 Trigger code stability

Labels: `spec`

Depends on: M3.2, M14.2.

Reason codes are an enum in rta-spec. A new code is a version bump for the log.

Acceptance:

- [ ] Enum is the source.
- [ ] Log format names the version.

## M103 — Threshold change diff

### M103.1 Threshold change diff

Labels: `spec`

Depends on: M3.2, M36.1.

A threshold literal change in the spec requires a fixture update and a notes update. CI checks both.

Acceptance:

- [ ] Rule is beside the spec gate.
- [ ] A lone threshold edit fails.

## M104 — Spec vector file

### M104.1 Spec vector file

Labels: `spec`

Depends on: M7.3, M58.1.

Vectors live in spec/vectors.json. The oracle and the fixtures share them.

Acceptance:

- [ ] One vector file.
- [ ] Replay reads it.

## M105 — Spec eval budget

### M105.1 Spec eval budget

Labels: `spec`

Depends on: M4.3, M3.2.

Spec eval has its own budget inside the deadline, start at 2 ms. Over budget is Revert even if the result is pass.

Acceptance:

- [ ] Budget is config.
- [ ] Slow-spec test reverts.

## M106 — Interpreter pin review

### M106.1 Interpreter pin review

Labels: `repo`

Depends on: M0.2, M64.1.

The pin is reviewed when it changes. The review note is the replay result.

Acceptance:

- [ ] dependencies.md says so.
- [ ] No floating interpreter requirement.

## M107 — Spec comment rule

### M107.1 Spec comment rule

Labels: `spec`

Depends on: M3.3.

A threshold without a comment naming the hazard id fails review. The check can be a lint script or a checklist row.

Acceptance:

- [ ] Rule is in spec-notes.
- [ ] Existing thresholds are commented.

## M108 — Reason code registry

### M108.1 Reason code registry

Labels: `spec`

Depends on: M102.1 if present else M3.2.

docs/reason-codes.md is the registry. Indication, log, and spec use the same strings.

Acceptance:

- [ ] One table.
- [ ] HMI doc links it.

## M109 — Verdict mapping review

### M109.1 Verdict mapping review

Labels: `spec`

Depends on: M1.2, M58.1 if present else M1.2.

The mapping table is reviewed when the enum changes. Pass, Inhibit, Revert stay the only variants.

Acceptance:

- [ ] Review line is in verdicts.md.
- [ ] No fourth variant in the crate.

## M110 — Dialect pin

### M110.1 Dialect pin

Labels: `io`

Depends on: M0.2, M22.1.

The dialect choice is repeated in the ICD and the dependency note. A mismatch fails review.

Acceptance:

- [ ] Both docs name the same dialect.
- [ ] Adapter uses that dialect only.

## M111 — Message rate limit

### M111.1 Message rate limit

Labels: `io`

Depends on: M6.1.

Outbound heartbeats stay at 1 Hz. Setpoints stay at the tick rate. A faster loop is a defect.

Acceptance:

- [ ] Rates are in the ICD.
- [ ] A test counts outbound heartbeats.

## M112 — Unsupported message drop

### M112.1 Unsupported message drop

Labels: `io`

Depends on: M6.2.

Unknown inbound messages are counted and dropped. They do not change state.

Acceptance:

- [ ] Counter is a log field.
- [ ] Garbage message test leaves ages unchanged.

## M113 — Command ack handling

### M113.1 Command ack handling

Labels: `io`

Depends on: M6.4.

A mode command waits for an ack until the watchdog bound. No ack is a repeated mode command, not a setpoint.

Acceptance:

- [ ] ICD names the ack.
- [ ] No-ack test sends mode only.

## M114 — Mode ack timeout

### M114.1 Mode ack timeout

Labels: `io`

Depends on: M113.1.

Timeout does not clear Revert and does not resume setpoints.

Acceptance:

- [ ] Test covers timeout.
- [ ] Setpoint count stays zero.

## M115 — Setpoint silence on revert

### M115.1 Setpoint silence on revert

Labels: `io`

Depends on: M6.4, M5.2.

A revert tick emits no setpoint bytes. The sink test counts bytes, not just the enum.

Acceptance:

- [ ] Byte count assertion exists.
- [ ] Inhibit may still emit navigation bytes.

## M116 — Heartbeat component id

### M116.1 Heartbeat component id

Labels: `io`

Depends on: M6.1.

Outbound heartbeat uses the configured component id. A test locks the value.

Acceptance:

- [ ] Test exists.
- [ ] Config is the only source.

## M117 — Link loss versus heartbeat loss

### M117.1 Link loss versus heartbeat loss

Labels: `io`

Depends on: M6.2.

The two ages are separate fields and separate reason codes. A test fails one without failing the other.

Acceptance:

- [ ] Two fixtures.
- [ ] Reason codes differ.

## M118 — Captured link replay

### M118.1 Captured link replay

Labels: `io`

Depends on: M7.2, M6.2.

A captured MAVLink file can rebuild input records for replay. The tool is rta-replay or a sibling. It does not transmit.

Acceptance:

- [ ] Tool refuses a send flag.
- [ ] A short capture is a fixture.

## M119 — Dialect mismatch startup

### M119.1 Dialect mismatch startup

Labels: `io`

Depends on: M110.1.

A peer dialect marker the adapter does not understand refuses startup. It does not guess.

Acceptance:

- [ ] Test feeds a mismatch.
- [ ] No setpoint is sent.

## M120 — Tracker sample clock

### M120.1 Tracker sample clock

Labels: `tracker`

Depends on: M15.2, M23.1.

Sample time is the monotonic host time at receive, not the frame timestamp. A frame clock step does not shrink tracker age.

Acceptance:

- [ ] Note is in the tracker crate.
- [ ] Test steps the frame clock back.

## M121 — Single-track rule

### M121.1 Single-track rule

Labels: `tracker`

Depends on: M15.2, M35.1.

Two detections become a miss, not a pick. The port emits confidence 0.

Acceptance:

- [ ] Fixture covers two tracks.
- [ ] Switch never sees a pair.

## M122 — Confidence range check

### M122.1 Confidence range check

Labels: `tracker`

Depends on: M2.1, M15.2.

Out-of-range confidence is a miss. The record constructor already rejects it. The tracker must not send it.

Acceptance:

- [ ] Tracker test clamps to a miss.
- [ ] Constructor test still rejects 1.1.

## M123 — Range non-negative check

### M123.1 Range non-negative check

Labels: `tracker`

Depends on: M15.2.

A negative range is a miss. The stub focal-length range is labeled a stub in the log.

Acceptance:

- [ ] Test covers a negative.
- [ ] Stub label is in the format doc.

## M124 — Hold-limit review

### M124.1 Hold-limit review

Labels: `tracker`

Depends on: M2.2, M15.3.

The hold limit changes only in a change that also updates the timing doc and a fixture.

Acceptance:

- [ ] Gate is written next to the spec gate.
- [ ] A lone hold-limit edit fails review.

## M125 — Detector timeout

### M125.1 Detector timeout

Labels: `tracker`

Depends on: M15.2, M4.3.

Detector time is inside the tracker, not the host deadline. A slow detector ages the sample. It does not stall the tick.

Acceptance:

- [ ] Timeout path exists.
- [ ] Host deadline test does not call the detector.

## M126 — Pipeline restart bound

### M126.1 Pipeline restart bound

Labels: `tracker`

Depends on: M15.1.

A dead pipeline restarts at most once per config interval. Further death is a missed sample.

Acceptance:

- [ ] Bound is config.
- [ ] Restart storm test stops restarting.

## M127 — Frame drop metric

### M127.1 Frame drop metric

Labels: `tracker`

Depends on: M86.1.

Dropped frames increment a counter in the tracker log. The host log stores the last count, not the frames.

Acceptance:

- [ ] Metric name is documented.
- [ ] No frame bytes in the host log.

## M128 — No frame in the host log

### M128.1 No frame in the host log

Labels: `tracker`

Depends on: M7.1, M15.2.

The tick log schema has no image field. A review check fails if one is added without a version bump and an explicit out-of-scope reversal.

Acceptance:

- [ ] Schema doc says so.
- [ ] Format version is unchanged by this task.

## M129 — Tracker dependency fence

### M129.1 Tracker dependency fence

Labels: `tracker`

Depends on: M15.1, M5.1.

cargo tree on rta-switch has no gstreamer. CI runs that check.

Acceptance:

- [ ] Job or script exists.
- [ ] Failure output names the crate.

## M130 — Packet index review

### M130.1 Packet index review

Labels: `assurance`

Depends on: M16.3.

The packet index is reviewed when a gap file changes. The checklist is a line in the release doc.

Acceptance:

- [ ] Release doc links the index.
- [ ] A stale link is a review fail.

## M131 — Gap review note

### M131.1 Gap review note

Labels: `assurance`

Depends on: M47.1.

Open items are copied into the packet at release, not summarized away. The release checklist says copy, not close.

Acceptance:

- [ ] Checklist verb is copy.
- [ ] Open-item file is the source.

## M132 — Known-problem list

### M132.1 Known-problem list

Labels: `assurance`

Depends on: M39.1.

docs/known-problems.md repeats residual risks in operator language. It links the hazard ids.

Acceptance:

- [ ] File exists.
- [ ] Runbook links it.

## M133 — Residual risk owner column

### M133.1 Residual risk owner column

Labels: `assurance`

Depends on: M39.1.

The owner column stays blank or says unassigned. This repository does not fill in a name as if the risk were accepted.

Acceptance:

- [ ] Column exists.
- [ ] No personal name is invented.

## M134 — Non-claim sentence check

### M134.1 Non-claim sentence check

Labels: `docs`

Depends on: M10.3, M16.3.

A checklist confirms the non-claim sentences are still in the packet and the assurance-gap note. A release fails review if they are gone.

Acceptance:

- [ ] Checklist row exists.
- [ ] Sentences are quoted in the checklist.

## M135 — Public-document citation check

### M135.1 Public-document citation check

Labels: `docs`

Depends on: M16.1, M14.3.

Citations name ASTM F3269-21, DO-178C, DO-333, and DoDD 3000.09 with dates. A citation without a date is a review fail.

Acceptance:

- [ ] Citation list is in docs/references.md.
- [ ] No certification claim uses those names.

## M136 — Out-of-scope sentence check

### M136.1 Out-of-scope sentence check

Labels: `docs`

Depends on: M14.3.

The out-of-scope list still names arming circuits, fuzes, safe-and-arm devices, and release actuators. A release review confirms the sentences.

Acceptance:

- [ ] Checklist row exists.
- [ ] Task file still ends with that list.

## M137 — Issue-to-heading check

### M137.1 Issue-to-heading check

Labels: `repo`

Depends on: M0.1.

A script lists ### headings and can be compared to open issues. It does not have to call GitHub. The output is docs/heading-index.md generated in CI or checked in.

Acceptance:

- [ ] Index exists.
- [ ] M40 through M139 appear.

## M138 — Milestone title check

### M138.1 Milestone title check

Labels: `repo`

Depends on: M137.1.

GitHub milestone titles for M20 through M39 match the heading text. Later milestones are headings in this file even if a GitHub milestone object does not exist yet.

Acceptance:

- [ ] Note is in docs/milestone-list.md.
- [ ] Heading text is the source.

## M139 — Task file table of contents

### M139.1 Task file table of contents

Labels: `repo`

Depends on: M137.1.

The top of docs/engineering-tasks.md links every milestone heading from M0 through M139. A missing heading fails the index script.

Acceptance:

- [ ] Contents list exists.
- [ ] M40-M139 are on it.


## M140 — Autonomy gaps

Nine issues for the gaps that bite before the paperwork tail. Same fail-closed rules. A new fault maps to Inhibit or Revert, never to a quieter Pass.

### M140.1 Pilot override beats the companion

Labels: `switch`

Depends on: M5.2, M6.4, M22.1.

A stick deflection, a flight-controller mode change off guided or offboard, or a configured RC takeover beats the companion in the same tick. The switch latches Inhibit until the pilot returns the aircraft to the guided mode and a config flag says the companion may resume. Revert still wins over that latch.

Work:

1. Read the flight controller's reported mode and RC override bit every tick.
2. On override, emit no setpoints and no commit. Do not fight the mode change with a recovery command unless the override is lost and the spec already says Revert.
3. Latch `pilot_hold` until the resume condition. No public clear other than that condition.
4. Reason code `pilot_override`.

Acceptance:

- [ ] A mode change off guided in a sink test suppresses setpoints in that same tick.
- [ ] The following tick stays inhibited without a resume.
- [ ] A Revert hazard during pilot hold still commands recovery only if the pilot has released the sticks. Document that rule in docs/verdicts.md.

### M140.2 Energy to rally

Labels: `spec`

Depends on: M2.1, M3.2, M97.1.

Fix age and a fence do not know if the rally is still reachable. Add `time_to_rally_s` and `endurance_s` to the input record. The spec trips `revert` while endurance still exceeds time-to-rally plus the margin in config. Tripping after the rally is unreachable is a failed test.

Work:

1. Endurance comes from the flight controller's battery or fuel estimate, aged like any other sample. Missing endurance is zero, which reverts.
2. Time-to-rally is distance to the validated rally divided by a config cruise speed, plus the mode-change latency assumption.
3. Threshold lives in the spec. Host only publishes the two numbers.
4. Reason code `energy`.

Acceptance:

- [ ] Fixture: endurance one second above the margin passes, one second below reverts.
- [ ] Missing battery sample reverts.
- [ ] The trip happens while the rally is still inside endurance.

### M140.3 Estimator health gate

Labels: `spec`

Depends on: M2.1, M3.2, M27.1.

Age is not health. Add `estimator_ok` from the flight controller's fail flag or innovation gate. A fresh position with `estimator_ok` false is `revert`. The fence, the barrier, and the shadow net all consume position, so this bit is the independent trip #66 asked for.

Work:

1. Map the dialect's estimator-unhealthy or innovation-fail field. If the dialect has no such field, startup fails closed rather than assuming healthy.
2. Spec stream `estimator_bad`. It is part of `revert`.
3. Reason code `estimator`.
4. Update the common-mode note: the remaining hole is a healthy flag that is wrong, and that stays a residual.

Acceptance:

- [ ] Fresh fix, estimator flag false, verdict Revert.
- [ ] Dialect with no flag refuses startup.
- [ ] docs/common-mode.md names this stream as the control.

### M140.4 Mission phase

Labels: `switch`

Depends on: M5.1, M14.1.

The complex function submits a phase with the setpoint: `search`, `track`, `commit`, `abort`. The monitor can see it. Commit is legal only in phase `commit`, with verdict Pass, a grant, and no inhibit stream. A setpoint that wanders into range during `search` or `track` cannot set the commit flag.

Work:

1. Extend the request trait with phase.
2. An unknown or missing phase is `search`.
3. Spec input `phase_commit`. Inhibit if commit is requested and phase is not commit.
4. Reason code `phase`.

Acceptance:

- [ ] Search phase inside commit range does not emit commit.
- [ ] Commit phase with a failed grant does not emit commit.
- [ ] Abort phase clears a pending commit request in the same tick.

### M140.5 Abort a commit already sent

Labels: `authority`

Depends on: M14.2, M33.1, M140.4.

Withdraw inhibits the next tick. That does not cancel a commit the flight controller already accepted. Add an abort command the adapter can send on withdraw, phase abort, or Revert. DoDD 3000.09 terminate-the-engagement is this command, not a hope that the next setpoint is quieter.

This is a mode or mission-item cancel on the existing link. It is not an arming circuit, a fuze, or a release actuator.

Work:

1. On withdraw, phase abort, or Revert, send the dialect's abort or mission-cancel if one exists. If none exists, the ICD says so and the residual is written in docs/authority.md.
2. Log `abort_sent` separately from inhibit.
3. An abort that is not acked retries until the watchdog bound, and does not resume commit.

Acceptance:

- [ ] Withdraw after a commit tick produces an abort message in the sink.
- [ ] No commit setpoint follows that abort.
- [ ] A dialect with no abort message is an explicit residual, not a silent pass.

### M140.6 Coast quality in denied navigation

Labels: `spec`

Depends on: M2.1, M3.2, M140.3.

Position arriving as a message is not navigation. Add `nav_source` and `coast_ok`. Sources are gnss, visual, inertial. Inertial-only is not a commit source. `coast_ok` is false when the coast time exceeds a spec limit or the visual-odometry health bit is false.

Work:

1. Host publishes the source and the coast age. It does not decide.
2. Spec: commit inhibit on inertial-only or coast not ok. Revert if coast is not ok and fix is also stale.
3. Reason codes `coast`, `nav_source`.
4. No visual-odometry implementation in this task. The port is the health bit. A missing bit is coast not ok.

Acceptance:

- [ ] Inertial-only with a valid grant does not commit.
- [ ] Visual source with health false inhibits commit.
- [ ] Missing nav source inhibits commit.

### M140.7 Protected-entity inhibit

Labels: `authority`

Depends on: M14.1, M35.1, M121.1.

A valid grant and a confident track are not sufficient. Add a no-strike list: ids or regions hashed into the grant. A track whose classification is unknown, a second detection in the same window, or a match on the no-strike list inhibits commit.

Work:

1. List is part of the signed grant. A list the host cannot parse inhibits.
2. Two detections in one window set `multi_track`. That inhibits, matching the single-track miss but at the authority layer so a tracker bug cannot bypass it.
3. Unknown classification inhibits. There is no default foe.
4. Reason codes `no_strike`, `multi_track`, `unknown_class`.

Acceptance:

- [ ] Match on the list inhibits despite confidence 0.9 and a valid grant.
- [ ] Two detections inhibit.
- [ ] Unknown class inhibits.

### M140.8 Airborne deconfliction

Labels: `spec`

Depends on: M2.3, M11.1, M140.4.

Nothing in the existing issues looks at another aircraft. Add `traffic_ok`, true only when no cooperating track is inside a config cylinder. Loss of the traffic source sets `traffic_ok` false. Commit is inhibited. Revert if the intruder is inside the recovery cylinder.

This is a keep-out around reported traffic, not a pursuit.

Work:

1. Source is a MAVLink traffic message or an empty source. Empty is not ok if config says traffic is required.
2. Spec streams `traffic_inhibit` and `traffic_revert`.
3. The barrier may consume the same cylinder as a constraint. It may not ignore a false `traffic_ok`.
4. Reason code `traffic`.

Acceptance:

- [ ] Intruder inside the commit cylinder inhibits commit.
- [ ] Intruder inside the recovery cylinder reverts.
- [ ] Required source missing inhibits.

### M140.9 Grant window versus monotonic time

Labels: `authority`

Depends on: M14.1, M23.1, M33.1.

The tick clock is monotonic. The grant window is wall time. Own the conversion. At startup, record the offset between wall time and the monotonic clock. Expiry is computed from that offset. A wall-clock step does not extend a grant. A negative offset or a missing wall clock at startup refuses the grant path.

Work:

1. `grant_remaining_ms` is a host output, computed once per tick from the frozen offset.
2. Spec inhibits when remaining is zero or negative.
3. A wall step larger than a config bound latches grant inhibit for the process.
4. Reason code `grant_time`.

Acceptance:

- [ ] Wall clock jumping forward does not lengthen the remaining time.
- [ ] Wall clock jumping back does not revive an expired grant.
- [ ] Missing wall clock at startup means no commit, and setpoints may still pass.

## Still out of scope





- Arming circuits, fuzes, safe-and-arm devices, and release actuators.
- A claim of DO-178C certification or of an authorizing-official signature.
- Inventing a fence the aircraft is not inside.
