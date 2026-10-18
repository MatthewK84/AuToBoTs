# Lint set

Clippy already denies warnings on the workspace. The switch crate adds a set. The host library and binaries deny the same three plus `clippy::panic`, `clippy::unreachable`, `clippy::todo`, and `clippy::unimplemented` outside tests; release builds abort on panic (`docs/panics.md`).

| Lint | Rule |
| --- | --- |
| `clippy::unwrap_used` | Forbidden. A deliberate unwrap in `rta-switch` fails CI. |
| `clippy::expect_used` | Forbidden outside tests. Tests may use it. |
| `clippy::indexing_slicing` | Forbidden on the command path. A slice index can panic. |

## Gap

Lints are not a proof. They do not catch a wrong verdict, a bad threshold, a missing grant check, or a spec that flipped. Those are tests and the Kani harness. A clean lint run is not evidence that the switch is correct.
