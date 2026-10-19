#!/usr/bin/env bash
# Simulated target timing for the Jetson Orin Nano 8GB, and the same run on a real board.
#
#   bash scripts/target-sim.sh [qemu] [host-sim]   simulated evidence only (default: both)
#   REAL_TARGET=1 bash scripts/target-sim.sh real  on the board itself (docs/target-timing.md)
#
# Every artifact says what it is: evidence=simulated with measured_on=qemu-aarch64 or
# measured_on=host-sim. Nothing here is a measurement on the board unless the `real` run
# was made on the board, and rta-target refuses that label for an emulated or scaled run.
#
# Environment:
#   TICKS        evaluations per run (default 10000; 50 ms paced ticks, so about 8.5 min)
#   MODE         paced (default) or tight
#   OUT_DIR      where the artifacts go (default target-sim)
#   CPU          CPU to pin to with taskset (default: the last online CPU)
#   HOST_MHZ     host CPU clock for the slowdown model (default: from /proc/cpuinfo)
#   TARGET_MHZ   modelled A78AE clock (default 960, the Orin Nano 8GB 7W preset; 1510 is 15W)
#   IPC_PENALTY  per-clock penalty of an A78AE against the host core (default 1.5)
#   ALLOW_OVER   1: exit 0 even when a run was over the deadline (the CI smoke job; a shared
#                runner's timing is not evidence either way)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

ticks=${TICKS:-10000}
mode=${MODE:-paced}
out_dir=${OUT_DIR:-target-sim}
cpu=${CPU:-$(($(nproc) - 1))}
target_mhz=${TARGET_MHZ:-960}
ipc_penalty=${IPC_PENALTY:-1.5}
host_mhz=${HOST_MHZ:-$(awk -F: '/^cpu MHz/ {printf "%d", $2; exit}' /proc/cpuinfo 2>/dev/null || true)}
host_mhz=${host_mhz:-2400}
today=$(date -u +%F)
runs=("$@")
if [[ ${#runs[@]} -eq 0 ]]; then
  runs=(qemu host-sim)
fi
mkdir -p "$out_dir"

# Real-time settings, where this machine allows them. Each one is recorded, not assumed.
rt=()
rt_note="sched=not-requested"
if chrt -f 80 true 2>/dev/null; then
  rt+=(chrt -f 80)
  rt_note="chrt=fifo80"
else
  rt_note="chrt=denied"
fi
if command -v taskset >/dev/null; then
  rt+=(taskset -c "$cpu")
  rt_note="$rt_note taskset=$cpu"
fi
# rta-target does not call mlockall; see docs/target-timing.md.
rt_note="$rt_note mlockall=no"

note() {
  # Append the model and run settings to an artifact, as comment lines.
  local file=$1 extra=$2
  printf '# run: %s %s\n' "$rt_note" "$extra" >>"$file"
}

status=0
for run in "${runs[@]}"; do
  case "$run" in
    qemu)
      command -v qemu-aarch64 >/dev/null || { echo "target-sim: qemu-aarch64 not installed (apt-get install qemu-user)"; exit 1; }
      CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=${CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER:-aarch64-linux-gnu-gcc} \
        cargo build --locked --release -q -p rta-host --bin rta-target --target aarch64-unknown-linux-gnu
      file="$out_dir/qemu-aarch64.txt"
      # qemu user mode translates each aarch64 instruction on the host. It does not model
      # an A78AE pipeline, clock, or cache, so no slowdown is applied on top.
      "${rt[@]}" qemu-aarch64 -L /usr/aarch64-linux-gnu \
        target/aarch64-unknown-linux-gnu/release/rta-target \
        --ticks "$ticks" --mode "$mode" --measured-on qemu-aarch64 --evidence simulated \
        --models jetson-orin-nano-8gb --slowdown 1 --date "$today" --out "$file" || status=$?
      note "$file" "emulator=$(qemu-aarch64 --version | head -n1 | tr ' ' '_')"
      ;;
    host-sim)
      cargo build --locked --release -q -p rta-host --bin rta-target
      slowdown=$(awk -v h="$host_mhz" -v t="$target_mhz" -v p="$ipc_penalty" 'BEGIN { s = h / t * p; if (s < 1) s = 1; printf "%.2f", s }')
      file="$out_dir/host-sim.txt"
      "${rt[@]}" target/release/rta-target \
        --ticks "$ticks" --mode "$mode" --measured-on host-sim --evidence simulated \
        --models jetson-orin-nano-8gb --slowdown "$slowdown" --date "$today" --out "$file" || status=$?
      note "$file" "model: slowdown=host_mhz/target_mhz*ipc_penalty host_mhz=$host_mhz target_mhz=$target_mhz ipc_penalty=$ipc_penalty"
      ;;
    real)
      # Only on the board, by an operator who says so. See docs/target-timing.md for the
      # RT kernel, isolcpus, nvpmodel and jetson_clocks steps that come first.
      if [[ "${REAL_TARGET:-}" != 1 || "$(uname -m)" != aarch64 ]]; then
        echo "target-sim: real needs REAL_TARGET=1 on the aarch64 board itself"
        exit 1
      fi
      cargo build --locked --release -q -p rta-host --bin rta-target
      file="$out_dir/real-target.txt"
      "${rt[@]}" target/release/rta-target \
        --ticks "$ticks" --mode "$mode" --measured-on "${BOARD_LABEL:-jetson-orin-nano-8gb}" \
        --evidence real-target --slowdown 1 --date "$today" --out "$file" || status=$?
      power=$(nvpmodel -q 2>/dev/null | tr '\n' ' ' | tr -s ' ' '_' || true)
      note "$file" "nvpmodel=${power:-unknown} kernel=$(uname -r)"
      ;;
    *)
      echo "target-sim: unknown run $run (qemu, host-sim, real)"
      exit 1
      ;;
  esac
  grep -q '^evidence=' "$file"
  echo "target-sim: wrote $file"
done
# rta-target exits 2 when a (modelled) evaluation is over the deadline. The deadline is not
# raised; the script reports it.
if [[ $status -eq 2 ]]; then
  echo "target-sim: a run had evaluations over the deadline; deadline unchanged"
  if [[ "${ALLOW_OVER:-}" == 1 ]]; then
    status=0
  fi
fi
exit "$status"
