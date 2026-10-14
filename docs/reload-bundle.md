# Reload bundle

A reload bundle is a directory. It is not a hot reload. Accepting a bundle does not swap the running monitor. A rejection leaves the running monitor in place.

## Contents

- `monitor.lola`, the candidate spec
- `manifest`, two lines: `previous_hash` and `new_hash`
- `manifest.sig`, a detached pin of the manifest bytes
- `weights`, optional. A weight file in the bundle is not activated by acceptance.

`previous_hash` is the pin of the running spec. `new_hash` is the pin of `monitor.lola`. The pin is the same non-cryptographic hash the tick log uses for `spec_hash`.

## Gate

`accept` borrows the running spec and returns a new one only if every check passes.

1. A missing or empty `manifest.sig` is unsigned and is rejected. A signature that is not the pin of the manifest is unsigned and is rejected. The pin is not a qualified cryptographic signature.
2. `monitor.lola` must pass the M3.1 parser (`load_spec`) before anything can be swapped. An unparsable spec is rejected.
3. A `new_hash` that is not the pin of `monitor.lola`, or a `previous_hash` that is not the running spec, is a hash mismatch and is rejected.

Rejection does not write the candidate into the running monitor. The caller keeps the monitor it already has. Swap is M13.2.

## Quarantine

Swap is not immediate. The candidate is built beside the running spec and replayed on the `must_hold` fixtures and on the last logged ticks. A spec that flips a `must_hold` fixture is refused. A shadow eval over the deadline is refused. Refusal leaves the command path on the running spec.

A fault mid-swap latches `Revert` and cannot leave the command path on the new spec. There is no public `clear_fault`. The latch clears only when the process is on a spec that has passed quarantine.

On-aircraft learning may write a candidate weight file. It may not point the active path at that file. Inference keeps using the active weights. Activation is a bundle swap. A successful swap logs the old hash, the new hash, and the tick number.
