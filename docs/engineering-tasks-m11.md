# AuToBoTs engineering tasks — expanded scope

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

## Still out of scope

- Arming circuits, fuzes, safe-and-arm devices, and release actuators.
- A claim of DO-178C certification or of an authorizing-official signature.
- Inventing a fence the aircraft is not inside.
