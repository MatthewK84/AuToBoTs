# SITL harness

The stack is PX4 SITL. The onboard MAVLink port is `udp:127.0.0.1:14540`, the same endpoint as `config/example.toml` and `config/sitl.toml`.

`scripts/sitl.sh` checks that those endpoints match, then brings the port up. If `PX4_DIR` points at a built PX4 tree it starts that binary. Otherwise it binds the same port with `scripts/sitl_peer.py` so the host can still connect and send a heartbeat. The host is `rta-sitl`, which loads the SITL config, opens the example endpoint, sends one heartbeat, and logs a nominal hover.

The example polygon is one meter on a side, smaller than the stopping margin at the configured speed. The SITL fence is a 200 meter square with the same speed, acceleration, and latency, so the hover point is inside the margin. Ages are 10 ms, under the spec limits. The script fails unless `sitl.log` contains `verdict=pass`.

## Scripted trips

`scripts/sitl-trips.sh` runs three cases and reads the tick log. A ground station is not the check.

- Fence breach writes `fc_report=rtl` and `verdict=revert` with `command=mode`.
- A stopped heartbeat writes `verdict=revert`.
- A weak track inside commit range writes `verdict=inhibit` and `commit=0`. The log must not contain a commit command.
