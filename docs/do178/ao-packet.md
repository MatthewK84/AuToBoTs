# Authorizing-official packet index

This is an index. It is not a certification claim, not an authority finding, and not a signature block. This repository does not fill one.

It does not say this repository meets DoDD 3000.09, DoDI 8430.01, DO-178C, or ASTM F3269-21.

## Planning data

- [PSAC](psac.md)
- [Software Development Plan](sdp.md)
- [Software Verification Plan](svp.md)
- [Software Configuration Management Plan](scmp.md)
- [Software Quality Assurance Plan](sqa.md)

The software level is a project choice, not an authority finding. Level C for the switch and the monitor. Level E for the untrusted complex function. The split is the point of ASTM F3269-21. An applicant can raise the level. This repository does not.

## Trace

- [Configuration index](sci.md)

- [Hazard trace](trace.md)
- [Hazard table](../hazards.md)
- [Kani log name](../kani.md)

Verification results are the CI log and the Kani log, named in the trace. A missing `trace: Hn` tag fails CI.

## Authority model

- [Authority](../authority.md)
- [Standards gate](../standards-3000-09-8430-01.md)
- [Human review rule](../review.md)

Commit requires a valid grant. A mode change is not a grant. Revert beats a valid grant. This repository does not implement a weapon, a fuze, or a release actuator.

## Recovery and the complex function

Recovery is the flight controller. The switch selects return, loiter, or land. It does not fly the recovery.

The complex function is not pedigreed. It proposes a setpoint. It does not construct a command. ASTM F3269-21 is the enclosing architecture. The filter and the shadow network are complex functions. They are not a replacement for the recovery.

## Residual hazards

From [hazards.md](../hazards.md). These are not covered by H1 through H15.

- Target identity.
- Collateral.
- Rules of engagement, beyond the signed grant already required for commit.
- Tracker truth. Confidence is an input, not a claim that the track is the right object.
- Estimator truth beyond age and the estimator-ok bit. A fresh wrong position remains a residual.

## Known gaps

These are listed. They are not folded into a compliance sentence.

### M11.3 Barrier evidence

[Barrier evidence](../cbf-evidence.md). Shown: forward invariance of the backup set under the point-mass model, in tests. Not shown: invariance under the real aircraft, wind, or a wrong fence. Residual model error is unmeasured. The filter is a complex function. It is not a replacement for the recovery.

### M12.2 Weight pin and budget

[Shadow network](../shadow-net.md). Weights are a file with a recorded hash. Load failure keeps the primary. The shadow exists only if the measured power and the deadline still close. The named board is the Orin Nano. That measurement is missing. The deadline is not raised. See [target](../target.md) and [deadline](../deadline.md). Issue 33 is closed with a production-test hook. See `docs/production-hooks.md`.

### M13 Learning and spec hot reload

[Reload bundle](../reload-bundle.md). An unsigned reload, a fixture regression, a swap timeout, a learned file without a bundle, and a fault mid-swap are H11 through H15. They are hazards. On-aircraft learning is gated by the bundle. It is not a claim that a learned change is safe.

### M16.2 and M17.2 Tool qualification

[Tool qualification](tool-qualification.md). M17.2 names the independent check for each unqualified tool. `rtlola-interpreter` and Kani automate verification and could fail to detect an error. DOT/FAA/TC-17/67 quotes DO-178C §12.2.2. They are Criterion 2 or 3. Neither has a qualification kit. Replay and Kani are evidence. They are not qualified-tool results. Those objectives still need review or an independent test.

## Other open measurements

- [Tracker timing](../tracker-timing.md). Issue 49 is closed with a production-test hook. See `docs/production-hooks.md`.
- Evidence artifacts, when a tag build produces them: SBOM and provenance record.
- SITL and scripted trips are developmental test evidence. They are not operational test.
