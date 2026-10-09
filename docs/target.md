# Target measurement

The companion-class board is the NVIDIA Jetson Orin Nano 8GB. The deadline is 5 ms, from `config/sitl.toml`. This note does not change that deadline.

Date: 2026-10-09.

The 10-minute eval ran on the build host, not on the Orin. It is the available stand-in until the named board is measured. The run took 512,611,400 samples. Worst-case eval duration was 60.839 ms. That exceeds the 5 ms deadline, so issue 33 stays open. The deadline is not raised in code.

CPU over the run was 96 percent. The power note is an order of magnitude for the Orin Nano module, about 10 W, not a wattmeter reading from this host.

Pausing the eval loop past three 50 ms ticks made the watchdog command recovery. The sink line was `mode Rtl`.

The raw line is in `crates/rta-host/fixtures/target-timing.txt`.

The follow-up review is in `docs/deadline.md`. A paced host run stayed under the deadline. That does not close this measurement. The Orin run is still missing.
