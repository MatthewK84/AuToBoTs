//! `read::apply` on any blob at any receive time. A panic, an out-of-bounds read, or a
//! touched field it does not own is a finding.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rta_host::read::{apply, PartialRecord};

fuzz_target!(|data: &[u8]| {
    let (now, blob) = data.split_at(data.len().min(8));
    let mut now_bytes = [0u8; 8];
    now_bytes[..now.len()].copy_from_slice(now);
    let now_ms = u64::from_le_bytes(now_bytes);
    let mut record = PartialRecord {
        track_conf: Some(0.5),
        range_m: Some(12.0),
        ..PartialRecord::default()
    };
    let before = record.clone();
    let result = apply(blob, now_ms, &mut record);
    assert_eq!(record.track_conf, Some(0.5));
    assert_eq!(record.range_m, Some(12.0));
    if result.is_err() {
        assert_eq!(record, before);
    }
});
