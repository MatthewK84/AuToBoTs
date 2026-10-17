# Tool qualification

This is a note, not a claim. DO-178C does not let an unqualified tool close an objective by itself. None of these tools has a qualification kit. Kani is not a qualified tool. `rtlola-interpreter` is not a qualified tool. The trace generator is not a qualified tool.

DOT/FAA/TC-17/67 quotes DO-178C §12.2.2. A tool that automates verification and could fail to detect an error is Criterion 2 or Criterion 3. For Level D, table 12-1 puts those at TQL-5. This repository recommends Level C for the switch and the monitor. The same criteria apply. The level is a project choice, not an authority finding.

## Rows

One row per tool used to produce verification evidence. The objective is what the tool is asked to support. The independent check is what covers that objective while the tool is unqualified.

| Tool | Objective asked to support | Independent check while unqualified |
| --- | --- | --- |
| `rtlola-interpreter` | Monitor spec verification. The Parasoft ebook names Tables A-3 through A-7. | Golden replay fixtures, `cargo run -p rta-host --bin rta-replay`. |
| Kani | Switch invariant on `decide`. DO-333 is the hook. The DO-333 annex tables are not in hand, so no FM.A objective is claimed as satisfied. | Switch table tests, including `decide_variants_are_exclusive`. |
| Trace generator (`untraced`) | Trace completeness: every hazard has a `trace: Hn` tag next to its named test. | The host test `an_untraced_hazard_fails_ci` reads the tags. A missing tag fails CI. |

Replay and Kani are evidence. They are not qualified-tool results. Those objectives still need review or an independent test. This gap is not folded into a compliance sentence.
