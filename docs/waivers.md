# Waivers

Each waiver names the check, the reason, and the condition that removes it. No silent allow.

| Check | Reason | Expires when |
| --- | --- | --- |
| RUSTSEC-2026-0195 | `mavlink-bindgen` 0.18 uses `Reader`, not `NsReader`. The advisory says a plain `Reader` is not affected. `cargo update -p quick-xml --precise 0.41.0` cannot be selected because that crate requires `quick-xml` ^0.39. | `mavlink-bindgen` accepts `quick-xml` >= 0.41 |
| RUSTSEC-2026-0194 | Same blocked upgrade. Consumer is the bindgen build script, not the flight binary. | Same |
| priority-queue LGPL-3.0 OR MPL-2.0 | Transitive from `rtlola-interpreter` 0.11.0. Copyleft stays denied everywhere else. | The interpreter drops this crate |
| Unicode-3.0 | OSI license on `unicode-ident`, pulled by `proc-macro2`. Not copyleft. | Not a waiver. Allowed. |
