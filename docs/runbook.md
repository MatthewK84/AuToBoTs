# Runbook

The host has no `clear_fault` step. A latched interpreter fault clears only by restarting the process.

## Inhibit versus Revert

`Inhibit` keeps the aircraft in the current mode. The switch sends setpoints with `commit` cleared. The operator sees the commit bit go to zero in the tick log. No recovery mode is commanded.

`Revert` commands the configured recovery mode. The switch sends `COMMAND_LONG` `MAV_CMD_DO_SET_MODE` when the reported mode is not already recovery. The bench custom mode is 6 for rtl, 5 for loiter, and 4 for land. A second revert tick sends nothing once the reported mode matches. The operator sees `verdict=revert` in the tick log and the mode change on the flight controller.

`Pass` forwards the complex-function request, including commit, only when that request carries commit.

## Pull the tick log

The path is `log.path` in the host config. The example is `rta.log`. Copy that file while the host is running. It is append-only. The first line is `# rta-log 1 spec_hash=<16 hex digits>`. Do not truncate it to clear a fault.

Check a saved log against the spec with:

```
cargo run -q -p rta-host --bin rta-replay -- <log> spec/monitor.lola
```

## Interpreter fault

A latched interpreter fault stays latched. Stop the host and start it again. That restart is the only clear. There is no `clear_fault` call and no operator command that unlatches it.

## Spec edit

A spec edit requires a golden-trace review in the same change. Golden traces live in `crates/rta-host/fixtures/golden`. CI runs `rta-replay` on that directory. Do not ship a spec change without that review. There is no hot reload.
