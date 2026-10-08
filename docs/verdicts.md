# Verdicts

The enum in `rta-spec` is the map. `Revert` beats `Inhibit` beats `Pass`. The host computes revert first and does not consult inhibit when revert is true.

| Verdict | Effect |
| --- | --- |
| Pass | Forward the latest setpoint request if one exists. Commit is allowed only when the grant and phase checks also pass. |
| Inhibit | Drop commit. Navigation setpoints may still forward, and only if no revert hazard is true. Inhibit is not a license to keep a commit command. |
| Revert | Emit no setpoint. Command the recovery mode already chosen in config. |

Recovery mode is `rtl`, `loiter`, or `land`, from `config/example.toml`. The switch receives that mode. It does not contain a default `"RTL"` that ignores config.

H9 is unreachable: a command cannot be both setpoints and a mode change. The table maps it to no verdict because the type forbids it.

| Hazard | Verdict |
| --- | --- |
| H1 fence exit | Revert |
| H2 stale fix | Revert |
| H3 stale command link | Revert |
| H4 stale heartbeat | Revert |
| H5 weak track inside commit range | Inhibit |
| H6 non-finite input | Revert |
| H7 monitor timeout | Revert |
| H8 interpreter fault | Revert, latched |
| H9 setpoints and mode change | Unreachable |
| H10 log write failure | Revert, latched |
