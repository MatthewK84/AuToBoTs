# SBOM

DoWI 8430.01 section 3.5.b: an accurate SBOM is generated and maintained for covered software.

The evidence job (`scripts/evidence.sh`) requires `artifacts/sbom.cdx.json` on a tag. A missing file fails the evidence job, not the compile job. The SBOM is CycloneDX generated from `Cargo.lock`.

It names the workspace crates and their direct dependencies, including `rtlola-interpreter`, `mavlink`, and any GStreamer binding that is actually linked. A crate that is not in the lockfile is not in the SBOM.

This note is the acceptance record for M141.1. The generation step itself is a residual until the tag build produces the file.
