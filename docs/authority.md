# Commit authority

Commit is a software conjunction. It does not close a circuit, arm a device, or command a release mechanism. Those are outside this repository.

DoDD 3000.09 is the policy reference. The design leaves a path for an operator to withhold or withdraw judgment. Absence of a grant is withhold. A revoked nonce is withdraw. A system that cannot meet the grant window terminates the commit and does not continue it.

## Same-tick conjuncts

`authorize` allows commit only when all of these are true in the same tick:

- the spec verdict is `Pass`
- the filter did not reject
- a signed operator grant is on hand
- the grant time window contains `now_ms`
- the grant fence hash matches the aircraft fence, the aircraft is inside that fence, and the range is within `max_range_m`
- the grant nonce is not revoked

A missing grant, an expired grant, a wrong fence hash, a revoked nonce, a bad signature, a filter rejection, and a non-pass verdict are `Inhibit`. `Revert` beats a valid grant. Verification failure is `Inhibit`, not a crash.

The grant is a signed record: issuer, not-before, not-after, fence hash, max range, nonce, and a detached pin. The pin is not a qualified cryptographic signature.

The commit flag is set only in the switch. The filter crate does not name it and does not depend on the switch. The tracker does not construct `Command::Commit`.
