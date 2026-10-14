# Specification layout

`spec/monitor.lola` is the only predicate source. CI parses it through `rtlola-frontend`. A syntax error fails the test job.

Required output streams are `revert` and `inhibit`. The names live in `rta_spec::REQUIRED_OUTPUTS`. A spec missing either fails load, before arming, before a socket opens.

A running spec changes only through a signed bundle that passes quarantine. An unsigned, unparsable, or mismatched bundle is H11 and is refused. A candidate that flips a `must_hold` fixture is H12 and is refused. A process restart remains the clear for a latched fault.
