# Frame ingress

`rta-tracker` owns frame ingress. The host does not decode. The switch has no video dependency.

## Pipeline

The pipeline string lives in config. It names a `udpsrc` or a `v4l2src`, a decode element, and an `appsink`.

```
udpsrc port=5600 ! decodebin ! appsink drop=true max-buffers=1
```

`parse_pipeline` rejects a string missing any of those. A pipeline that fails to start is a startup failure of the tracker process. The host then sees a missing tracker sample. M2.2 already maps a missing sample to confidence 0.

This crate does not link `libgstreamer`. The string is the GStreamer description. The sink contract is what the tests run. Linking the library would put a video dependency where the acceptance says there is none.

## Sink

The appsink keeps the latest buffer. `pull_latest` returns that buffer. A pad older than the tracker hold limit, or a killed source, yields `track_conf = 0` through the existing port. Age grows from the last buffer time.

A recorded fixture stream is `crates/rta-tracker/fixtures/stream.txt`.

## Detector

A detector behind the sink emits one track: confidence, range estimate, sample time. Range is a stub from bbox height and a config focal length. The stub is labeled a stub. It is not a measured range.

Output crosses into the host only through the M6.3 `Tracker` trait. `accept` takes a `TrackSample`. It does not take a frame. There is no path from a frame to `decide`.

Stale detector output uses the M2.2 hold policy: unseen, or older than the hold, is confidence 0. That is not a second policy.
