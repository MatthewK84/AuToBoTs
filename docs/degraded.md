# Degraded modes

What still flies when one input is lost. Commit is never allowed on a degraded row.

| Row | Navigation | Commit | Recovery |
| --- | --- | --- | --- |
| Lost tracker | allowed | not allowed | not commanded |
| Lost grant | allowed | not allowed | not commanded |
| Shadow only | allowed | not allowed | not commanded |
| Filter timeout | not allowed | not allowed | commanded |
| Stale link | not allowed | not allowed | commanded |

Lost tracker and lost grant are inhibit. Setpoints may continue. Commit does not. Shadow only keeps the shadow proposal and drops commit. Filter timeout and a stale link are revert. Recovery is commanded.

The matrix matches the verdict tests. Inhibit clears commit. Revert has no commit field.
