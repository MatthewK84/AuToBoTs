#!/usr/bin/env bash
# PX4 SITL harness. The onboard port is the example config port.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
example=$(grep -E 'endpoint = ' config/example.toml)
sitl=$(grep -E 'endpoint = ' config/sitl.toml)
test "$example" = "$sitl"
grep -q '127.0.0.1:14540' scripts/sitl.sh
peer=
if [[ -n "${PX4_DIR:-}" && -x "$PX4_DIR/build/px4_sitl_default/bin/px4" ]]; then
  echo "sitl: starting PX4 from $PX4_DIR"
  (cd "$PX4_DIR" && build/px4_sitl_default/bin/px4 -d etc) &
  peer=$!
else
  echo "sitl: PX4 binary not present, binding the onboard port"
  python3 scripts/sitl_peer.py &
  peer=$!
fi
cleanup() { kill "$peer" 2>/dev/null || true; }
trap cleanup EXIT
sleep 0.5
cargo run -q -p rta-host --bin rta-sitl -- config/sitl.toml
grep -q 'verdict=pass' sitl.log
echo "sitl: nominal hover logged pass"
