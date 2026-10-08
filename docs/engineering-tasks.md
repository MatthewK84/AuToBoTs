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

## Still out of scope


- Arming circuits, fuzes, safe-and-arm devices, and release actuators.
- A claim of DO-178C certification or of an authorizing-official signature.
- Inventing a fence the aircraft is not inside.
