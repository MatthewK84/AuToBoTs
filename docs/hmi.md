# Human-machine indication

DoDD 3000.09 requires the interface to be understandable to a trained operator: which actions the operator must perform, and which actions the system will perform. This is that contract. It is not a certification claim.

The indication rides the link the adapter already owns. It is a MAVLink `STATUSTEXT`. There is no second link.

## Who does what

| Actor | Action |
| --- | --- |
| Operator | Issues a grant. Withdraws a grant. Changes mode. Takes the stick. |
| System | Evaluates the tick. Indicates `commit_armed`, `commit_inhibited`, or `reverted`, with a reason code. Selects the recovery mode. Does not invent a grant. |

A mode change is not a grant. A grant is not a mode change.

## States

- `commit_armed`: the tick may commit. A valid grant is present and no inhibit row is true.
- `commit_inhibited`: the tick does not commit. The reason code says why.
- `reverted`: the tick is recovery. The flight controller flies it. A grant message in the same tick does not overwrite this indication.

## Reason codes

Every reason code from M14.2.

| Code | Meaning |
| --- | --- |
| `fence` | Fence exit, including the stopping-distance margin. |
| `fix` | Stale position fix. |
| `link` | Stale command link. |
| `heartbeat` | Stale flight-controller heartbeat. |
| `weak_track` | Weak track inside commit range. |
| `no_grant` | No valid grant. |
| `grant_expired` | Grant window has closed. |
| `grant_revoked` | Operator withdrew the grant. |
| `fence_hash` | Grant fence hash does not match. |

## Same tick

A `Revert` indication cannot be overwritten by a grant message in the same tick. The grant may be recorded. The indication stays `reverted`.
