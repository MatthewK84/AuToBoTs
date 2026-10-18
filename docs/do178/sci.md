# Software configuration index

This is a configuration index. It names hashes, not floating branch tips. It is not a certification claim.

## Current row

| Item | Hash or version |
| --- | --- |
| Spec `spec/monitor.lola` | `ae398e85f30cd1e8723bbef36ad934b3651017e6d987eeedbc7fddf229bd6553` |
| Switch crate content | `8ef36e2c13b88ae726738d52da06723be20e7ad52127db46f0b8b069dfb052ac` |
| Switch crate last change | `802feaa2f982b53f46f237e78804e47a4c014aff` |
| Config `config/example.toml` | `747ff29188a50757aed426688ee7229695d09a62bdd8a947845bc67c3d2781db` |
| Rust toolchain | `1.98.1` |
| `Cargo.lock` | `b9f7bf855f2a02f3471b045294dea211bb8af4ca95d1ed1895a9aa99a0b26804` |
| Runner image | `ubuntu-latest`, floating. Gap. |
| `rtlola-interpreter` | `0.11.0` |
| `rtlola-frontend` | `0.8.0` |
| `mavlink` | `0.18.0` |
| `kani-verifier` | `0.68.0` |

Tool versions are from `docs/dependencies.md`. The switch content hash is the hash of the crate sources. The last-change value is the commit that last touched the crate. Neither is a branch tip.

## Parameter data

`config/example.toml` is the reviewed parameter item. It is not a flight file. A flight config that differs from it is a named file with its own hash in this index, not an untracked edit. Unknown keys fail closed.

## Known gaps

- M11.3: barrier invariance is shown under the point-mass model only. Residual model error is unmeasured.
- M12.2: the shadow exists only if power and deadline still close. The Orin measurement is missing. The deadline is not raised.
- M13: on-aircraft learning is gated by a bundle. It is not a claim that a learned change is safe.
- M16.2 and M17: `rtlola-interpreter` and Kani are not qualified tools. See `docs/do178/tool-qualification.md`.
- Issue 33 and issue 49 are closed with a production-test hook. See `docs/production-hooks.md`.
- The runner image is `ubuntu-latest`. It is floating. It cannot be pinned to a digest yet. Two runs of one commit still hash the same `Cargo.lock`.

## Bundle swap

A reload bundle from M13 is a new index row, not an edit to an old row. The old row stays. The new row records the new spec hash, the new switch content hash if the crate changed, and the new config hash if the config changed.

The bundle swap procedure says the index updates in the same change as the swap. A swap that lands without a new index row is incomplete. See `docs/reload-bundle.md`.
