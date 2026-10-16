# Pipeline timing

The named board is the NVIDIA Jetson Orin Nano 8GB, from M10.1. This note does not change `tracker_hold_ms`.

Date: 2026-10-16.

Pipeline string:

```
udpsrc port=5600 ! decodebin ! appsink drop=true max-buffers=1
```

The measurement ran on the build host, not on the Orin. 10,000 detector-stub samples, worst 15 us, wall under 1 ms. The raw line is in `crates/rta-tracker/fixtures/timing.txt`. It times the detector stub, not a GStreamer decode. This crate does not link `libgstreamer`. The run is the available stand-in until the named board is measured.

Hold limit: 200 ms, from `config/sitl.toml` and `config/example.toml`. The hold was not raised. M2.2 was not revised.

Issue 49 stays open until a decode-plus-detect measurement on the Orin Nano is under that hold, or the pipeline is simplified or the detector is replaced on that board. The hold does not grow to hide a miss.
