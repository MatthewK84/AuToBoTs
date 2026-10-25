# Third-party notices

Direct dependencies. `cargo deny` is the check. A new copyleft dependency fails CI. It does not get a waiver inside a feature task.

| Crate | License | Used by | Notes |
| --- | --- | --- | --- |
| rtlola-interpreter | Apache-2.0 | rta-host | Spec interpreter. |
| rtlola-frontend | Apache-2.0 | rta-host | Spec parser. |
| mavlink | MIT/Apache-2.0 | rta-host | Common dialect, UDP and serial. |
| serde | MIT OR Apache-2.0 | rta-host, rta-spec | Derive. |
| serde_json | MIT OR Apache-2.0 | rta-spec | |
| toml | MIT OR Apache-2.0 | rta-host | Config. |
| ordered-float | MIT | rta-host | |
| ctrlc | MIT/Apache-2.0 | rta-host | |
| sha2 | MIT OR Apache-2.0 | rta-host, rta-docs | |
| priority-queue | LGPL-3.0 OR MPL-2.0 | transitive | Deny exception. |
| serialport | MPL-2.0 | transitive via mavlink | Deny exception. |

GStreamer is a pipeline string in `rta-tracker`. This repository does not link libgstreamer. There is no GStreamer binding dependency.

## Deny config

`deny.toml` allows MIT, Apache-2.0, BSD-3-Clause, and Unicode-3.0. The exceptions are `priority-queue` and `serialport`. A new copyleft crate that is not in this table fails the deny job.
