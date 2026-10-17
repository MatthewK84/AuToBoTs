# Tool qualification

This is a note, not a claim. Neither tool has a qualification kit.

DOT/FAA/TC-17/67 quotes DO-178C §12.2.2. A tool that automates verification and could fail to detect an error is Criterion 2 or Criterion 3. For Level D, table 12-1 puts those at TQL-5. This repository recommends Level C for the switch and the monitor. The same criteria apply. The level is a project choice, not an authority finding.

- `rtlola-interpreter` evaluates the monitor spec. It is a verification tool. It is not qualified.
- Kani proves `decide`. It is a verification tool. It is not qualified.

Objectives that therefore still need review or an independent test:

- Verification of the monitor spec. The Parasoft ebook names Tables A-3 through A-7 for verification. Replay is evidence. It is not a qualified-tool result.
- The switch invariant on `decide`. Kani is evidence. It is not a qualified-tool result. DO-333 is the hook. The DO-333 annex tables are not in hand, so no FM.A objective is claimed as satisfied.

This gap is not folded into a compliance sentence.
