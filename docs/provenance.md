# Build provenance

DoWI 8430.01 section 3.5.c: covered software is produced by a secure, automated, observable build, from trusted sources, protected from tampering.

A tagged build records four fields in the configuration index:

| Field | Source |
| --- | --- |
| commit SHA | `git rev-parse HEAD` |
| lockfile hash | SHA-256 of `Cargo.lock` |
| toolchain | the pinned `rust-toolchain.toml` channel |
| CI run URL | the GitHub Actions run that produced the tag |

The record is machine-readable (`artifacts/provenance.json`). An untagged local build is not provenance. A tag that lacks the four fields is marked a gap, not a pass.

This note is the acceptance record for M141.2.
