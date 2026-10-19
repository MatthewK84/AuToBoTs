# Production-test hooks

These measurements are not claimed. The issues that owned them are closed with this hook. The run happens when production testing starts, on the named board.

## Orin target measurement

Board: NVIDIA Jetson Orin Nano 8GB. Deadline: 5 ms. The host stand-in is in `docs/target.md`, dated 2026-10-09. Worst-case eval was 60.839 ms on the host. The deadline is not raised.

Simulated timing does not close this hook. `docs/target-timing.md` holds qemu-aarch64 and host-model runs labelled `evidence=simulated`, and the steps for the board run.

Hook: a 10-minute paced eval on the Orin, with CPU and a wattmeter power note, recorded in `docs/target.md` with a new date. If the worst case exceeds 5 ms, the deadline stays and a follow-up is opened. Do not quietly raise `deadline_ms`.

## Orin pipeline timing

Pipeline string: `udpsrc port=5600 ! decodebin ! appsink drop=true max-buffers=1`. Hold: 200 ms. The host stand-in is in `docs/tracker-timing.md`, dated 2026-10-16. It times the detector stub. This crate does not link `libgstreamer`.

Hook: a decode-plus-detect measurement on the Orin, recorded in `docs/tracker-timing.md` with a new date. If it exceeds 200 ms, the hold does not grow. The pipeline is simplified or the detector is replaced.
