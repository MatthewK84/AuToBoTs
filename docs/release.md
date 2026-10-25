# Ground release

Ordered list. A release tag points at the commit that passed every item. No in-flight learning bundle is part of a ground release.

| # | Item | Maps to |
| --- | --- | --- |
| 1 | Tests green | CI job `compile`: `cargo test --locked --workspace` |
| 2 | Coverage artifact present | CI job `coverage`: artifact `switch-coverage` |
| 3 | Index hashes match | `docs/do178/sci.md` against the files named in the row |
| 4 | Target measurement dated | `docs/target.md` carries a date |
| 5 | Gaps copied into the packet | `docs/do178/ao-packet.md` lists the open gaps |
| 6 | Spec notes current | `docs/spec-notes.md` matches `spec/monitor.lola` |

A release tag is cut from main at a green commit. A tag from a dirty tree is refused. Hot-reload bundles are not release artifacts.

## Tag procedure

1. The tree is clean. `git status --porcelain` is empty. A dirty index is refused.
2. The commit is on main and the CI run for that commit is green.
3. `git tag -a vX.Y.Z <commit>` and push the tag.
4. Bundle files (`weights`, candidate specs, quarantine output) are not tagged.

The tag names the commit. It does not name a branch tip.
