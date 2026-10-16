//! Host stand-in for decode-plus-detect latency. Not an Orin measurement.

use rta_tracker::{emit, Detection, Focal};
use std::time::Instant;

fn main() {
    let focal = Focal {
        length_px: 800.0,
        target_height_m: 1.0,
    };
    let samples = 10_000u64;
    let mut worst_us = 0u64;
    let started = Instant::now();
    for index in 0..samples {
        let start = Instant::now();
        let _ = emit(
            Detection {
                confidence: 0.8,
                bbox_height_px: 20.0,
                sample_time_ms: index,
            },
            focal,
        );
        let eval_us = start.elapsed().as_micros() as u64;
        if eval_us > worst_us {
            worst_us = eval_us;
        }
    }
    println!(
        "board=host-stand-in samples={samples} worst_us={worst_us} wall_ms={} pipeline=\"udpsrc port=5600 ! decodebin ! appsink drop=true max-buffers=1\"",
        started.elapsed().as_millis()
    );
}
