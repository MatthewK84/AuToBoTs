#!/usr/bin/env bash
# Stress the real-time tests: the runtime and recovery tests of rta-host, RUNS times idle and
# RUNS times beside HOGS busy loops, and count what failed and what the host pause guard
# excused (docs/stress.md).
#
#   bash scripts/stress.sh          (RUNS=60 HOGS=16 FILTER="runtime:: recovery::")
#
# Prints, per phase, the failures and the runs where a test failed across a host pause and was
# run again. Exits non-zero if any run failed. Leaves no busy loop behind, even on Ctrl-C.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
runs=${RUNS:-60}
hogs=${HOGS:-16}
filter=${FILTER:-runtime:: recovery::}
out=${OUT_DIR:-target/stress}
rm -rf "$out"
mkdir -p "$out"

bin=$(cargo test --locked -p rta-host --lib --no-run 2>&1 | sed -n 's/.*Executable .*(\(.*\))$/\1/p' | tail -n 1)
[[ -x "$bin" ]] || { echo "stress: no test binary"; exit 1; }
# A copy, so a rebuild during the run cannot swap the binary under it.
cp "$bin" "$out/rta-host-tests"

pids=()
stop_hogs() {
  if ((${#pids[@]})); then
    kill "${pids[@]}" 2>/dev/null || true
    wait "${pids[@]}" 2>/dev/null || true
  fi
  pids=()
}
trap stop_hogs EXIT INT TERM

failed=0
phase() {
  local name=$1 fails=0 paused=0 i
  for i in $(seq "$runs"); do
    # shellcheck disable=SC2086 # the filter is a list of test-name filters
    if ! RTA_HOST_PAUSE_LOG="$out/$name-$i.paused" "$out/rta-host-tests" $filter >"$out/$name-$i.log" 2>&1; then
      fails=$((fails + 1))
      echo "stress: $name run $i failed ($out/$name-$i.log)"
    fi
    [[ -s "$out/$name-$i.paused" ]] && paused=$((paused + 1))
  done
  echo "stress: $name: $fails/$runs runs failed; $paused/$runs runs had a test fail across a host pause and run again"
  failed=$((failed + fails))
}

phase idle
for _ in $(seq "$hogs"); do
  sh -c 'while :; do :; done' &
  pids+=($!)
done
echo "stress: $hogs busy loops on $(nproc) CPUs"
phase load
stop_hogs
((failed == 0))
