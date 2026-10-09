# Deadline review

Issue 194 reviews the 2026-10-09 host measurement. The deadline stays 5 ms. `deadline_ms` is not raised. Issue 33 stays open until a Jetson Orin Nano 8GB run is under that deadline.

The 60.839 ms worst case came from a tight loop: 512,611,400 samples in 10 minutes at 96 percent CPU. That sample is a host scheduling stall, not a reason to change the spec. A paced run evaluates once per 50 ms tick and records eval time separately from the sleep. The deadline check uses that eval time.

The named board is still the NVIDIA Jetson Orin Nano 8GB. This host is not that board.

Paced run, 2026-10-09, one minute on the build host: 1,194 samples, worst eval 2.363 ms, none over the 5 ms deadline, CPU 0.1 percent. The watchdog still commanded `mode Rtl` when the loop was paused. That run is in `crates/rta-host/fixtures/paced-timing.txt`.

Decision: the deadline stays 5 ms. The 60.839 ms tight-loop sample is a host stall. It is not a spec change. Issue 33 remains open for an Orin Nano measurement.
