#!/usr/bin/env bash
# Scripted trips. The assertion is the tick log, not a ground station.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
grep -q '127.0.0.1:14540' config/example.toml
cargo run -q -p rta-host --bin rta-trips
grep -q 'verdict=revert' trips/fence.log
grep -q 'fc_report=rtl' trips/fence.log
grep -q 'command=mode' trips/fence.log
grep -q 'verdict=revert' trips/heartbeat.log
grep -q 'verdict=inhibit' trips/weak.log
grep -q 'commit=0' trips/weak.log
if grep -q 'command=commit' trips/weak.log; then
  echo "trips: weak track committed" >&2
  exit 1
fi
echo "trips: fence, heartbeat, and weak track asserted from the log"
