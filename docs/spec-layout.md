# Specification layout

`spec/monitor.lola` is the only predicate source. CI parses it through `rtlola-frontend`. A syntax error fails the test job.

Required output streams are `revert` and `inhibit`. The names live in `rta_spec::REQUIRED_OUTPUTS`. A spec missing either fails load, before arming, before a socket opens.

There is no hot reload. A spec change is a process restart and a golden-trace review.
