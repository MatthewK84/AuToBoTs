//! One inbound datagram through the frame decoder, including the raw `COMMAND_ACK` path, and
//! the raw ack reader on the same bytes. A panic or more frames than the bound is a finding.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rta_host::mavlink_link::{command_ack, decode_frames, Frame, MAX_FRAMES_PER_DATAGRAM};

fuzz_target!(|data: &[u8]| {
    let mut frames = 0;
    decode_frames(data, |_, frame| {
        frames += 1;
        if let Frame::CommandAck(ack) = frame {
            let _ = (ack.command, ack.result);
        }
    });
    assert!(frames <= MAX_FRAMES_PER_DATAGRAM);
    let ack = command_ack(data);
    let at = |index: usize| data.get(index).copied().unwrap_or(0);
    assert_eq!(ack.command, u16::from_le_bytes([at(0), at(1)]));
    assert_eq!(ack.result, at(2));
});
