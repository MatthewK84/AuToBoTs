# Plan for Software Aspects of Certification

This is a repository planning document. It is not a certification claim and not an authority finding. An applicant, if any, can raise the software level. This repository does not.

## Standards named

- RTCA DO-178C, Software Considerations in Airborne Systems and Equipment Certification.
- RTCA DO-330, Software Tool Qualification Considerations.
- RTCA DO-333, Formal Methods Supplement to DO-178C and DO-278A.
- ASTM F3269-21, Standard Practice for Methods to Safely Bound Behavior of Aircraft Systems Containing Complex Functions Using Run-Time Assurance.

DO-178C and DO-330 are not public in full. The documents in hand are explanatory:

- Parasoft, *DO-178C Software Compliance for Aerospace & Defense*.
- Holloway and Graydon, *Explicate '78: Assurance Case Applicability to Digital Systems*, DOT/FAA/TC-17/67, January 2018.

No objective identifier is cited from memory. The quotes below are from those two documents.

From the Parasoft ebook, Software Planning Process, Section 4: "The objectives are clearly defined and captured in Table A-1 of the standard. There are seven objectives that must be satisfied based on the software level (A-D)." The ebook lists those as the software life cycle process, inter-relationships between processes, methods and tools, development standards, verification that the software satisfies development requirements, and verification of the organizations that perform the activities. It names the planning outputs as the PSAC, the Software Development Plan, the Software Verification Plan, the Software Configuration Management Plan, and the Software Quality Assurance Plan. It also names Tables A-1 through A-10.

From the same ebook: "Catastrophic results equate to failure of flight control software where an aircraft would go down and many lives would be lost. This would be classified as software level A." "Hazardous is a step down... software level B. The classification continues to go down to software level E where there's no safety concern if failure were to occur."

From DOT/FAA/TC-17/67, Appendix B, quoting DO-178C §12.2.2:

- Criterion 1. "A tool whose output is part of the airborne software and thus could insert an error."
- Criterion 2. "A tool that automates verification process(es) and thus could fail to detect an error, and whose output is used to justify the elimination or reduction of … verification process(es) other than that automated by the tool, or … development process(es) that could have an impact on the airborne software."
- Criterion 3. "A tool that, within the scope of its intended use, could fail to detect an error."

The report states that DO-330's purpose "is to provide tool qualification guidance." For Level D, table 12-1 specifies TQL-4 for Criterion 1 and TQL-5 for Criterion 2 or 3 only.

DO-333 is the hook for the Kani results on `decide` and for the RTLola replay argument. The DO-333 annex tables are not in hand, so no FM.A objective identifier is cited.

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
