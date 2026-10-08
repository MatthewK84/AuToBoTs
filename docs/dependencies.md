# Dependency pins

Pinned on Rust 1.98.1. Exact crate versions. Checksums are the crates.io checksums in Cargo.lock.

| Crate | Version | Checksum | Crate that may depend on it |
| --- | --- | --- | --- |
| rtlola-interpreter | 0.11.0 | 14ad151e9d313e9587df7a1c0dd6058924dba50cbd8d797c38208116285649d2 | rta-host only |
| rtlola-frontend | 0.8.0 | 547d3da386bdc6ee7438859e10e2fc2a06018e913f2c00cc15963206d19de793 | rta-host only |
| mavlink | 0.18.0 | ef539c358c31f69d47816dac709eebbcb733166ae1bcdeeb4463383c042e3e7c | rta-host only |
| kani-verifier | 0.68.0 | not in the default lock | rta-switch, feature `verify` only |

Dialect is `common` only. Default features are off, so `dialect-ardupilotmega` is not linked. M9 names PX4 SITL as the default harness, and the common dialect is the PX4 message set. ArduPilot would be a separate pin, not a second feature on this crate.

`cargo test --workspace` does not enable `verify`, so it does not build Kani. The solver is not required.

`deny.toml` allows MIT, Apache-2.0, and BSD-3-Clause. The advisory job is separate from compile so a network failure there does not hide a build failure.
