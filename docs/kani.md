# Kani harness

The harness is `crates/rta-switch/src/verify.rs`. It calls `decide`, `decide_input`, and `TickInput::try_new`. A missing verdict is a host convention and is not in this harness.

Proofs for `decide`, for every verdict, request, and recovery mode:

- A `Mode` command has no setpoint payload, and a `Setpoints` command has no mode payload. The variants are exclusive.
- `Revert` never yields `Setpoints`.
- `Inhibit` never yields a commit flag.

Numeric request fields are bounded to a byte so the solver finishes. The invariant is the command variant, not the magnitude.

Proofs for `decide_input`, for every phase, every flag, and every `f64` endurance and time to rally, including NaN, infinities, and negatives. The input is a struct literal, so it is not filtered by `TickInput::try_new`:

- `decide_input_commit_needs_every_gate`: `Commit` only with a grant, phase `Commit`, no override, no abort, a good estimator, coast ok, and finite, non-negative energy with endurance strictly above the time to rally.
- `decide_input_non_finite_energy_reverts`: a non-finite energy value never commits and, unless an override or abort wins first, is `Recovery` Revert.
- `decide_input_verdict_matches_command`: `Revert` is `Abort` or `Recovery`, `Inhibit` is `Navigation`, `Pass` is `Commit`, and only `Pass` carries a grant hash.
- `tick_input_try_new_admits_only_finite_energy`: `try_new` returns `Ok` only for finite, non-negative energy values.

Run it outside default CI:

```
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
cargo kani -p rta-switch
```

`kani-verifier` is an optional dependency of `rta-switch`, feature `verify`, off by default. `cargo test --workspace` does not invoke the solver. The Kani workflow is `.github/workflows/kani.yml`. It runs on pull requests, on push to `main`, and by hand. It is not a dependency of the default CI close job.
