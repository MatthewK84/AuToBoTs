# Target timing: simulated, and how to measure the real board

**Everything measured here is SIMULATED. None of it is a measurement on a Jetson Orin Nano 8GB.** No Jetson was on hand. The numbers below come from an emulator and from a scaled host run, and they are labelled that way in every artifact. They do not close the Orin target measurement in `docs/production-hooks.md`, and they do not change the 5 ms deadline.

## Evidence labels

`rta-target` (`crates/rta-host/src/bin/rta-target.rs`) writes one key=value line that says what kind of evidence it is. There is no `named_board` field any more; an earlier line named the board it was not run on.

| Field | Meaning |
| --- | --- |
| `evidence=simulated` | qemu user mode, or a host run scaled by the slowdown model. Never board timing |
| `evidence=host-standin` | a native host run, unscaled. Not any target board |
| `evidence=real-target` | an unscaled run of an aarch64 build on the board, declared by the operator with `--evidence real-target` |
| `measured_on` | where it ran: `qemu-aarch64`, `host-sim`, `build-host`, or the board label |
| `models` | the board the run stands in for. It is not a claim that the run happened there |
| `slowdown` | the factor applied to measured times; 1 means unscaled |

`rta-target` refuses `real-target` for a `qemu*` or `*sim*` label, for a slowdown above 1, and for a non-aarch64 build, and refuses `host-standin` for an emulated or scaled run. An artifact starts with a `# SIMULATED`, `# HOST STAND-IN` or `# REAL-TARGET` comment line. The document checks (`crates/rta-docs/src/host/target.rs`) fail CI if a timing fixture has no evidence label, names a board as `named_board` or `board`, labels an emulated or scaled run anything but simulated, or claims `real-target` at all: there is no board run yet, and adding one is a reviewed change to that check.

The two 2026-10-09 lines (`fixtures/target-timing.txt`, `fixtures/paced-timing.txt`) were relabelled from `board=`/`named_board=Jetson Orin Nano 8GB` to `evidence=host-standin measured_on=build-host models=jetson-orin-nano-8gb`. Their numbers are unchanged.

## What is measured

One evaluation of `spec/monitor.lola` on the SITL hover record per tick, through `Evaluator::tick`, the work the tick thread waits for up to `tick.deadline_ms`. In `paced` mode the loop sleeps to a 50 ms period (`tick.period_ms` in `config/sitl.toml`) and also records how late each tick woke. Percentiles are exact over one-microsecond bins. Each run is 10,000 ticks.

Not measured: the rest of the tick body (record assembly, the switch, encoding, the log write), the receive thread and the watchdog, MAVLink on a real link, and contention from other processes on the board (the tracker, a camera pipeline, the GPU). The full tick loop under qemu needs the fake flight controller and is left as an owner decision.

## The two simulations

`bash scripts/target-sim.sh` runs both and writes `target-sim/qemu-aarch64.txt` and `target-sim/host-sim.txt`. The committed copies are in `crates/rta-host/fixtures/target-sim/`.

**qemu-aarch64.** The `aarch64-unknown-linux-gnu` release build of `rta-target`, run under `qemu-aarch64` user mode on the build host. This runs the same aarch64 instructions the board would. It does not model an A78AE: qemu translates each block to host code, so its timing reflects the translator and the host, not the board's pipeline, clock or caches. The first evaluations include translation. No slowdown is applied on top.

**host-sim.** The native host build, with each measured time multiplied by a slowdown factor:

    slowdown = host_mhz / target_mhz * ipc_penalty

Assumptions, each overridable in the script's environment:

- The modelled CPU is the Orin Nano 8GB's Cortex-A78AE (six cores, 1.5 GHz class). Its clock depends on the nvpmodel preset: 1510.4 MHz with six cores at 15 W (mode 0), 960 MHz with four cores at 7 W (mode 1), per the NVIDIA Jetson Linux developer guide. The default `TARGET_MHZ=960` models the slower, 7 W preset.
- `host_mhz` is read from `/proc/cpuinfo` (2400 MHz on this build host, a virtualised Intel Xeon). A virtual machine's reported clock may not be its running clock.
- `ipc_penalty=1.5` is an assumed per-clock disadvantage of the A78AE against the host core: smaller caches (64 KB L1, 256 KB L2 per core, 2 MB L3 per cluster) and LPDDR5 memory latency. It is a judgement, not a measurement.
- Scaling is linear and never shrinks a time. It does not model cache misses that the host does not have, thermal throttling, DVFS ramp-up from idle, or interrupts on the board.

With the defaults the factor is 2400 / 960 * 1.5 = 3.75.

Real-time settings are applied where the machine allows them and recorded in a `# run:` line: `chrt -f 80` (SCHED_FIFO) was denied on this host, so both runs used the default scheduler; `taskset` pinned each run to one CPU. `rta-target` does not call `mlockall`; that needs `unsafe` and a `libc` dependency in `rta-host`, which is an owner decision.

## Results (SIMULATED)

2026-10-19, build host (8 vCPU virtualised Intel Xeon, 2.4 GHz reported), 10,000 paced ticks per run, the two runs side by side on CPUs 7 and 5. Times are modelled eval microseconds (for qemu, the emulated time unscaled). Deadline 5,000 us, period 50,000 us.

| Run | Slowdown | p50 us | p99 us | p99.9 us | Worst us | Over the 5 ms deadline | Worst wake lateness us |
| --- | --- | --- | --- | --- | --- | --- | --- |
| qemu-aarch64 | 1 | 54 | 119 | 271 | 675991 | 1 of 10000 | 5356537 |
| host-sim | 3.75 | 60 | 132 | 454 | 1791008 | 3 of 10000 | 5282536 |

- At p99.9 both runs are an order of magnitude under the deadline: 271 us under qemu and 454 us under the 3.75x model, against 5,000 us. The p50 is under 2 percent of the deadline and about 0.1 percent of the period.
- The worst cases are not the evaluation. Both processes, pinned to different CPUs, woke about 5.3 s late once (5356537 us and 5282536 us) and both had one evaluation stretched to 0.5 to 0.7 s: the signature of a pause of the whole virtual machine, not of either process. The host model multiplies that pause by 3.75. These samples are the build host's, not a property of the monitor, and they are why a simulated worst case cannot stand in for the board's.
- The first evaluation is cold: 2466 us under qemu (block translation), 68 us modelled on the host.
- The watchdog still commanded `mode Rtl` when the loop was paused, in both runs.
- Per the model, `rta-target` exited 2 (over the deadline) for both runs. The deadline is not raised.

Read these as a sanity check that the evaluation is far from the deadline on an aarch64 build and under a pessimistic linear model, not as the board's worst case. A real-target worst case comes only from the board run below.

## Running the same measurement on a real Jetson

On the board, with JetPack installed and the repository checked out:

1. Kernel: a PREEMPT_RT kernel (NVIDIA ships a real-time kernel package for Jetson Linux), and `isolcpus=5 nohz_full=5 rcu_nocbs=5` on the kernel command line so one A78AE core runs nothing else. Check with `uname -v` and `cat /proc/cmdline`.
2. Power and clocks: `sudo nvpmodel -m 0` (15 W) or `-m 1` (7 W), the preset the vehicle will fly, then `sudo jetson_clocks` to pin the clocks at that preset's maximum. Record `nvpmodel -q`.
3. Real-time scheduling and memory: run as a user allowed SCHED_FIFO (`chrt -f 80`) and with `ulimit -l unlimited`. `mlockall` is not in `rta-target` yet; until it is, note that page faults are possible.
4. Run: `REAL_TARGET=1 CPU=5 TICKS=10000 bash scripts/target-sim.sh real`. This builds natively, runs under `chrt -f 80 taskset -c 5`, and writes `target-sim/real-target.txt` with `evidence=real-target`, the board label, `nvpmodel` and the kernel in the `# run:` line. Run it again with the tracker and camera pipeline loaded, and for at least 10 minutes (`--minutes 10` on `rta-target`) for the production hook.
5. Power: a wattmeter reading alongside, for `docs/shadow-net.md`.
6. Record: commit the artifact under `crates/rta-host/fixtures/`, update `docs/target.md` and `docs/deadline.md` with the date, and change the `real-target` check in `crates/rta-docs/src/host/target.rs` in the same reviewed change. If the worst case exceeds 5 ms, the deadline stays and a follow-up is opened, as `docs/production-hooks.md` says.
