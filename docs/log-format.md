# Tick log

Append-only. One header, then one record per tick. The host opens this file before it opens a socket. An open failure exits non-zero. A write failure in flight latches revert.

## Header

```
# rta-log 1 spec_hash=<16 hex digits>
```

Version is `1`. `spec_hash` is a non-cryptographic hash of the spec file bytes.

## Record

Space-separated fields:

- `tick`
- `fence_ok` as 0 or 1
- `fix_age_ms`
- `link_age_ms`
- `fc_heartbeat_age_ms`
- `track_conf`
- `range_m`
- `verdict` as `pass`, `inhibit`, or `revert`
- `reasons` as a comma-separated list, empty if none
- `command` as `mode`, `setpoints`, or `idle` — the variant actually emitted
- `eval_ms`
- `spec_hash`

The file is flushed every `log.flush_every_n` records.

`rta-replay <log> <spec>` re-evaluates each record. A verdict mismatch prints the tick, expected verdict, actual verdict, and the input record, then exits non-zero.
