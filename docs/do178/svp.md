# Software Verification Plan

This is a repository planning document. It is not a certification claim.

## Level and standards

The software-level choice is in `docs/do178/psac.md`. It is a project choice, not an authority finding. Verification of the switch and the monitor follows the Level C recommendation. The complex function is Level E and is not verified as assured software. The split is the point of ASTM F3269-21.

DO-178C is the verification guidance named here. The Parasoft ebook names Tables A-3 through A-7 for verification. DO-333 is the hook for the Kani results on `decide` and for the RTLola replay argument. The DO-333 annex tables are not in hand, so no FM.A objective identifier is cited. DOT/FAA/TC-17/67 is the document in hand for tool qualification.

## Verification

CI runs `cargo fmt`, `cargo clippy`, `cargo test`, and the golden replay. Kani covers `decide`. Replay covers the monitor spec. A missing hazard tag is a verification gap, owned by M16.2.

## Hazards

Each of H1 through H15 has a named test in `docs/hazards.md`. Verification does not invent a second hold policy or a second deadline. H7 stays on `deadline_ms`. H12 stays on the quarantine fixture.
