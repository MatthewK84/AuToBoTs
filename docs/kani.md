# Kani harness

The harness is `crates/rta-switch/src/verify.rs`. It calls `decide` only. A missing verdict is a host convention and is not in this harness.

Proofs, for every verdict, request, and recovery mode:

- A `Mode` command has no setpoint payload, and a `Setpoints` command has no mode payload. The variants are exclusive.
- `Revert` never yields `Setpoints`.
- `Inhibit` never yields a commit flag.

Numeric request fields are bounded to a byte so the solver finishes. The invariant is the command variant, not the magnitude.

Run it outside default CI:

```
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
cargo kani -p rta-switch
```

`kani-verifier` is an optional dependency of `rta-switch`, feature `verify`, off by default. `cargo test --workspace` does not invoke the solver. The Kani workflow is `.github/workflows/kani.yml` and is not a dependency of the default CI close job.
