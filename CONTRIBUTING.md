# Contributing

This repository is fail-closed runtime assurance for an aircraft. A change is judged by what it does to the fail-closed path first and by everything else after. Read `docs/standards-3000-09-8430-01.md` ("What a code change must show") before the first change.

## One change per pull request

- One gap, fix, or feature per pull request, branched from the latest `main`. If it overlaps an open pull request, stack it on that branch and say so in the description.
- Fix forward. Never amend, rebase, or force-push a commit that has been pushed. A mistake is fixed by a new commit.
- Commit messages say what changed and why, in the imperative. A line `Issue: #N` on its own makes CI's `close-named-issue` job close issue N after the commit lands on `main` (`docs/ci.md`). Leave it out unless the commit really closes that issue.

## Checks before you push

Every one of these must pass locally. CI runs them too (`docs/ci.md`).

```sh
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -q -p rta-host --bin rta-replay -- crates/rta-host/fixtures/golden spec/monitor.lola
cargo deny check
bash scripts/coverage.sh
bash scripts/sitl-trips.sh
bash scripts/sitl.sh
cargo build --locked --release --workspace
TICKS=200 MODE=tight ALLOW_OVER=1 bash scripts/target-sim.sh qemu host-sim
```

The last line needs `qemu-user`, `gcc-aarch64-linux-gnu` and `libc6-dev-arm64-cross`. It is a smoke check of the simulated-timing path, not timing evidence (`docs/target-timing.md`).

CI also runs `bash scripts/sitl-px4.sh` (PX4 SITL, about 5 minutes the first time; needs `cmake`, `ninja-build` and `python3-venv`). Run it before a change to the runtime, the recovery latch, the gate or the spec.

Then `./scripts/wait-ci.sh` until CI is green.

A change to a timing-sensitive path (the runtime loop, the worker, the watchdog, the recovery latch) should also be stress-run. Run the test binaries repeatedly, idle and under CPU load, and post the pass counts on the pull request.

## Tests

- Tests are deterministic. No random seed: property tests use a fixed seed (`crates/rta-host/tests/properties.rs`). A timing bound leaves room for a loaded runner. A flaky test is a bug. Fix it; do not retry it.
- A test asserts something. A test name says what it shows, and the test shows it.
- A hazard control has a test tagged `// trace: Hn` in its flight module, listed in `docs/do178/trace.md`.

## Documents

- A document check lives in `crates/rta-docs`, not in a flight crate (`docs/doc-checks.md`). If you change what the code does, change the note in the same pull request; the check fails CI if they disagree.
- `rta-host` reads only the spec, a config, or a fixture at compile time.
- The tick log format is frozen at version `1`. Changing a field means a version bump and a golden-trace review (`docs/log-format.md`).
- A spec change needs a golden-trace review in the same change (`docs/runbook.md`).

## Dependencies

- Exact pins, listed in `docs/dependencies.md` with their checksums. A new dependency must pass `cargo deny check` (`deny.toml`).
- A change to `Cargo.lock` updates the lock hash in `docs/do178/sci.md`.
- A GitHub Action is pinned to a full commit SHA with its release in a comment (`docs/ci.md`).

## Owner decisions

Some choices belong to the owner, not the author: a new hazard row, a new degraded-matrix row that changes what is commanded, a raised deadline, a dropped check, a release tag. Do not make those in the change. List them in the pull request under "Owner decisions" and leave them for the owner.

## Review

Every change is reviewed. A change written or modified with AI assistance says so in its description and names its human reviewer. The developer stays accountable for it (`docs/standards-3000-09-8430-01.md`, section 3.6).

## License

Contributions are under the Apache License, Version 2.0 (`LICENSE`), the license every crate declares.
