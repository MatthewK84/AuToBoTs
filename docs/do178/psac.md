# Plan for Software Aspects of Certification

This is a repository planning document. It is not a certification claim and not an authority finding. An applicant, if any, can raise the software level. This repository does not.

## Standards named

- RTCA DO-178C, Software Considerations in Airborne Systems and Equipment Certification.
- RTCA DO-333, Formal Methods Supplement to DO-178C and DO-278A.
- ASTM F3269-21, Standard Practice for Methods to Safely Bound Behavior of Aircraft Systems Containing Complex Functions Using Run-Time Assurance.

DO-333 is the hook for the Kani results on `decide` and for the RTLola replay argument. The full Annex A and Annex FM objective tables are not in hand. No objective identifier is cited from memory.

Identifiers quoted from documents opened while writing this plan:

- DO-178C Table A-1, Software planning process, from the Parasoft overview of Section 4.
- DO-178C Table A-7 Objective 9, "Verification of additional code, that cannot be traced to Source Code, is achieved," from the Wikipedia summary of DO-178C.
- DO-333 Annex Table FM.A-3, FM.A-4, FM.A-5, FM.A-6, and FM.A-7, and Objective FM-10 in Table FM.A-7, from RTCA FTP1037.

## Software level

This is a project choice, not an authority finding.

- `rta-switch` and the monitor in `rta-host` are recommended at Level C.
- The untrusted complex function is recommended at Level E.

The split is the point of ASTM F3269-21. The switch and the monitor bound the complex function. The complex function is unassured by design, which is why conventional DO-178C artifacts are not claimed for it. Level E is the starting recommendation for that function. An applicant can raise either level. This repository does not.

## Hazards

The plans point at hazard ids H1 through H15 in `docs/hazards.md`.

- H1 fence exit, H2 stale fix, H3 stale link, H4 stale heartbeat: monitor and switch.
- H5 weak track inside commit range: monitor.
- H6 non-finite input, H7 deadline, H8 interpreter error, H9 command exclusivity, H10 log write failure: host and switch.
- H11 unsigned reload, H12 fixture regression, H13 swap timeout, H14 learned file without a bundle, H15 fault mid-swap: bundle and quarantine.

## Non-claim

This repository does not implement a weapon, a fuze, or a release actuator. Recovery is the flight controller. These documents do not fill a signature block.
