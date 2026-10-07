# AuToBoTs engineering tasks

Fail-closed runtime assurance for a Group 1–3 aircraft. The flight controller owns actuators and recovery. A Rust host owns I/O and the switch. An RTLola specification owns every predicate.

This is not an authorizing-official package, not a control-barrier filter, and not a claim that the tactical decision is correct. It bounds flight and commit.

Each `###` heading is one GitHub issue. Do the milestones in order. M3 and M4 can run in parallel after M2. M10 can start once M5 exists.

Frozen decisions, do not relitigate in a task:

- Thresholds live in `spec/monitor.lola`. The host has no numeric threshold constants.
- Any fault maps to `Revert`: late tick, interpreter error, missing verdict, NaN input, dead heartbeat, log write failure in flight.
- A latched interpreter fault clears only on process restart.
- Recovery is a mode the flight controller already honors: RTL, Loiter, or Land. Config chooses which. Code does not.
- The complex function cannot reach the MAVLink socket. It submits a request. The switch disposes of it.
- `Revert` beats `Inhibit` beats `Pass`.

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

## Out of scope

- Control-barrier or safety-filter synthesis.
- A second neural net as the backup controller.
- On-aircraft learning or spec hot reload.
- Weapons-release authority beyond the inhibit conjunction.
- DO-178C or an authorizing-official package.
- Video decode, tracker implementation, GStreamer.
