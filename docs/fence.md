# Fence

The host evaluates the polygon. The flight-controller fence trips after breach, and this monitor must trip early. The spec sees `fence_ok` only. It never sees raw vertices.

The trip line is inside the legal line by the stopping-distance margin. Margin is `v_max^2 / 2a_max` plus `tick.mode_change_latency_ms` times `v_max`. Those numbers stay in config. A missing, short, or degenerate polygon is `fence_ok = false`. The vertical cap is the same check.

`source = "fc"` is accepted as a config form and is not the early trip. The host polygon is the default.
