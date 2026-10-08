# Stale and missing samples

The host ages samples. The spec decides they are bad. Do not invert that.

`tick.tracker_hold_ms` is a host policy, not a spec threshold. It starts at the example-config value. A missing tracker sample holds the last confidence and range and sets an age. When that age exceeds the hold limit, the value presented to the spec is `track_conf = 0`. The last range is a log extra only. It is not a spec input that can still pass the commit check.

A missing position is not invented. `fix_age_ms` grows from the last accept time. A stale coordinate is not republished as fresh.

The first tick with no sample yet uses an age larger than any reasonable spec limit. The first verdict is `Revert` until fresh data arrives. Startup is not special-cased to `Pass`.
