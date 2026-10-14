# Commit authority

Commit is a software conjunction. This repository does not implement a weapon, a fuze, or a release actuator. Those are outside this repository. This note is a design trace, not a compliance claim.

DoDD 3000.09, January 25, 2023, is the policy reference. Paragraph 1.1.b.(7) says the directive does not apply to autonomous or semi-autonomous systems that are not weapon systems. This repository does not implement a weapon system.

## Policy sentences

Quoted from DoDD 3000.09, January 25, 2023:

- 1.2.a: "Autonomous and semi-autonomous weapon systems will be designed to allow commanders and operators to exercise appropriate levels of human judgment over the use of force."
- 1.2.a.(1)(b): "Complete engagements within a timeframe and geographic area, as well as other relevant environmental and operational constraints, consistent with commander and operator intentions. If unable to do so, the systems will terminate the engagement or obtain additional operator input before continuing the engagement."

Absence of a grant is the withhold path. A revoked nonce is the withdraw path. A window that cannot be met terminates the commit and does not continue it.

## Trace

Each conjunct maps to a test and to the paragraph it is meant to support. A missing row fails the switch test.

| Conjunct | Test | DoDD 3000.09 |
| --- | --- | --- |
| verdict is Pass | `revert_beats_a_valid_grant` | 1.2.a.(1)(b) terminate |
| filter did not reject | `a_rejected_filter_inhibits_commit` | 1.2.a.(1)(b) constraints |
| fresh operator grant | `no_grant_is_inhibit` | 1.2.a appropriate levels of human judgment |
| time window | `expired_grant_is_inhibit` | 1.2.a.(1)(b) timeframe |
| geographic window | `wrong_fence_hash_is_inhibit` | 1.2.a.(1)(b) geographic area |
| grant not revoked | `revoked_nonce_is_inhibit` | 1.2.a.(1)(b) additional operator input |
| mode change is not a grant | `a_mode_change_is_not_a_grant` | 1.2.a appropriate levels of human judgment |

`Revert` beats a valid grant. That is the terminate path in 1.2.a.(1)(b). A system that cannot meet the grant window terminates the commit and does not continue it.

## Same-tick conjuncts

`authorize` allows commit only when all of these are true in the same tick:

- the spec verdict is `Pass`
- the filter did not reject
- a signed operator grant is on hand
- the grant time window contains `now_ms`
- the grant fence hash matches the aircraft fence, the aircraft is inside that fence, and the range is within `max_range_m`
- the grant nonce is not revoked

A missing grant, an expired grant, a wrong fence hash, a revoked nonce, a bad signature, a filter rejection, and a non-pass verdict are `Inhibit`. Verification failure is `Inhibit`, not a crash.

The grant is a signed record: issuer, not-before, not-after, fence hash, max range, nonce, and a detached pin. The pin is not a qualified cryptographic signature.

The commit flag is set only in the switch. The filter crate does not name it and does not depend on the switch. The tracker does not construct `Command::Commit`.

## Operator path

The grant arrives on the existing link as a line the adapter parses into the token: `GRANT issuer=... not_before=... not_after=... fence_hash=... max_range=... nonce=... sig=...`. `WITHDRAW nonce=...` invalidates that nonce. The next tick is `Inhibit` for commit even if the aircraft is still inside the window.

A flight-controller heartbeat or `SET_MODE` is a mode change. The adapter does not invent a grant from it.

The operator indication is a logged line: `commit_armed`, `commit_inhibited` with a reason code, or `reverted`. Reason codes are the trigger names `fence`, `fix`, `link`, `heartbeat`, `weak_track`, plus `no_grant`, `grant_expired`, `grant_revoked`, and `fence_hash`.
