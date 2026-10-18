# CI

Two workflows: `.github/workflows/ci.yml` on every push and pull request, and `.github/workflows/kani.yml` on pull requests, on push to `main`, and by hand.

## Jobs

| Job | What it runs | Toolchain |
| --- | --- | --- |
| `compile` | `cargo fmt --check`, clippy `-D warnings` on all targets, `cargo test --locked --workspace` (document checks included), golden replay | 1.98.1 |
| `release` | `cargo build --locked --release --workspace`: the flight profile, `panic = "abort"` | 1.98.1 |
| `aarch64` | The same release build for `aarch64-unknown-linux-gnu`, the Jetson triple, linked with `gcc-aarch64-linux-gnu`. A cross-build check, not a run on the board | 1.98.1 |
| `sitl` | `scripts/sitl-trips.sh` and `scripts/sitl.sh` against the stand-in peer. `PX4_DIR` is unset, so no PX4 binary runs | 1.98.1 |
| `deny` | `cargo deny check` | action's own |
| `coverage` | `scripts/coverage.sh`, the switch coverage gate | 1.98.1 |
| `fuzz-smoke` | each `fuzz/` target, 200,000 runs on seed 1 (`docs/doc-checks.md`) | nightly-2026-10-18, by `+toolchain` only |
| `evidence` | SBOM and provenance, on tags only | none |
| `close-named-issue` | on push to `main` after `compile`, `deny`, `evidence`: closes the issue named by an `Issue: #N` line | none |
| `prove` (kani.yml) | `cargo kani -p rta-switch` | 1.98.1 |

## Minimum supported Rust

The minimum supported Rust is the pinned toolchain, 1.98.1, in `rust-toolchain.toml`. Every crate declares `rust-version = "1.98.1"`. There is no separate MSRV job: every stable job already builds on exactly that version, so a second job would build the same thing twice. An MSRV job becomes meaningful only if the pin and the declared minimum part ways.

## Pins

Every action is pinned to a full 40-character commit SHA, with the release it was taken from in a trailing comment. A tag can be moved; a commit cannot. To update an action, change the SHA and the comment together, in a reviewed change. `rta-docs` fails CI on a `uses:` line without a SHA.

| Action | Release |
| --- | --- |
| `actions/checkout` | v4.4.0 |
| `dtolnay/rust-toolchain` | the `1.98.1` ref, and `master` with an explicit `toolchain:` for the dated nightly |
| `EmbarkStudios/cargo-deny-action` | v2.1.1 |
| `actions/upload-artifact` | v4.6.2 |

`actions/checkout` runs with `persist-credentials: false` in every job, so no later step finds the token in `.git/config`.

The runner image is `ubuntu-latest`, still floating (`docs/do178/sci.md`).

## Permissions

Both workflows set `permissions: contents: read` at the top. No job can write to the repository, a release, a package, or a pull request.

`close-named-issue` is the only job with more: `contents: read` and `issues: write`, for `gh issue close`. It runs only on push to `main`. It reads the number only from a whole line `Issue: #N` (or `Issue: N`) in the commit message, so a mention of "Issue:" inside prose does not close anything. It still closes whatever issue number that line names. Whether to keep the job is an owner decision.
