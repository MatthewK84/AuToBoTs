//! Outbound commands. Mode and setpoints are separate messages. Idle sends nothing.
//!
//! Recovery modes use the PX4 custom mode from `px4_custom_mode.h`. RTL, loiter, and land are
//! sub-modes of the AUTO main mode. They are not main modes of their own.

use mavlink::dialects::common::{
    MavCmd, MavMessage, MavModeFlag, MavSeverity, PositionTargetTypemask, COMMAND_LONG_DATA,
    SET_POSITION_TARGET_LOCAL_NED_DATA, STATUSTEXT_DATA,
};
use mavlink::types::CharArray;
use rta_spec::RecoveryMode;
use rta_switch::authority::{indication_line, Indication};
use rta_switch::SwitchCommand;

pub fn to_message(command: &SwitchCommand, target_system: u8) -> Option<MavMessage> {
    match command {
        SwitchCommand::Mode { recovery } => Some(MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
            param1: f32::from(MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED.bits()),
            param2: f32::from(px4_mode(*recovery).main),
            param3: f32::from(px4_mode(*recovery).sub),
            param4: 0.0,
            param5: 0.0,
            param6: 0.0,
            param7: 0.0,
            command: MavCmd::MAV_CMD_DO_SET_MODE,
            target_system,
            target_component: 1,
            confirmation: 0,
        })),
        SwitchCommand::Setpoints {
            north, east, down, ..
        } => Some(MavMessage::SET_POSITION_TARGET_LOCAL_NED(
            SET_POSITION_TARGET_LOCAL_NED_DATA {
                time_boot_ms: 0,
                x: *north as f32,
                y: *east as f32,
                z: *down as f32,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                afx: 0.0,
                afy: 0.0,
                afz: 0.0,
                yaw: 0.0,
                yaw_rate: 0.0,
                type_mask: PositionTargetTypemask::empty(),
                target_system,
                target_component: 1,
                coordinate_frame: mavlink::dialects::common::MavFrame::MAV_FRAME_LOCAL_NED,
            },
        )),
        SwitchCommand::Idle => None,
    }
}

/// `PX4_CUSTOM_MAIN_MODE_ACRO` in `px4_custom_mode.h`. Never a recovery mode.
pub const PX4_MAIN_MODE_ACRO: u8 = 5;
/// `PX4_CUSTOM_MAIN_MODE_AUTO` in `px4_custom_mode.h`. Every recovery mode is under it.
pub const PX4_MAIN_MODE_AUTO: u8 = 4;
/// `PX4_CUSTOM_MAIN_MODE_OFFBOARD` in `px4_custom_mode.h`. Never a recovery mode.
pub const PX4_MAIN_MODE_OFFBOARD: u8 = 6;
/// `PX4_CUSTOM_SUB_MODE_AUTO_LOITER` in `px4_custom_mode.h`.
pub const PX4_SUB_MODE_AUTO_LOITER: u8 = 3;
/// `PX4_CUSTOM_SUB_MODE_AUTO_RTL` in `px4_custom_mode.h`.
pub const PX4_SUB_MODE_AUTO_RTL: u8 = 5;
/// `PX4_CUSTOM_SUB_MODE_AUTO_LAND` in `px4_custom_mode.h`.
pub const PX4_SUB_MODE_AUTO_LAND: u8 = 6;

/// Bit offset of `main_mode` in the PX4 `custom_mode` union.
const PX4_MAIN_MODE_SHIFT: u32 = 16;
/// Bit offset of `sub_mode` in the PX4 `custom_mode` union.
const PX4_SUB_MODE_SHIFT: u32 = 24;

/// PX4 main mode and sub-mode. `COMMAND_LONG` carries them in param2 and param3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Px4Mode {
    pub main: u8,
    pub sub: u8,
}

pub fn px4_mode(mode: RecoveryMode) -> Px4Mode {
    let sub = match mode {
        RecoveryMode::Rtl => PX4_SUB_MODE_AUTO_RTL,
        RecoveryMode::Loiter => PX4_SUB_MODE_AUTO_LOITER,
        RecoveryMode::Land => PX4_SUB_MODE_AUTO_LAND,
    };
    Px4Mode {
        main: PX4_MAIN_MODE_AUTO,
        sub,
    }
}

/// The packed `HEARTBEAT.custom_mode` PX4 reports when it is in this recovery mode.
pub fn custom_mode(mode: RecoveryMode) -> u32 {
    let px4 = px4_mode(mode);
    (u32::from(px4.sub) << PX4_SUB_MODE_SHIFT) | (u32::from(px4.main) << PX4_MAIN_MODE_SHIFT)
}

/// Decode a PX4 `HEARTBEAT.custom_mode`. Only AUTO with the RTL, loiter, or land sub-mode is
/// a recovery mode. Any other main mode, including offboard and acro, is not.
pub fn reported_mode(custom_mode: u32) -> Option<RecoveryMode> {
    let main = ((custom_mode >> PX4_MAIN_MODE_SHIFT) & 0xFF) as u8;
    let sub = ((custom_mode >> PX4_SUB_MODE_SHIFT) & 0xFF) as u8;
    match (main, sub) {
        (PX4_MAIN_MODE_AUTO, PX4_SUB_MODE_AUTO_RTL) => Some(RecoveryMode::Rtl),
        (PX4_MAIN_MODE_AUTO, PX4_SUB_MODE_AUTO_LOITER) => Some(RecoveryMode::Loiter),
        (PX4_MAIN_MODE_AUTO, PX4_SUB_MODE_AUTO_LAND) => Some(RecoveryMode::Land),
        _ => None,
    }
}

/// Indication on the link this adapter already owns. No second link.
pub fn indication_message(shown: Indication) -> MavMessage {
    let line = indication_line(shown);
    let bytes = line.as_bytes();
    let mut text = [0u8; 50];
    // `zip` stops at the shorter side: at most 50 bytes, never out of bounds.
    for (to, from) in text.iter_mut().zip(bytes) {
        *to = *from;
    }
    let severity = if shown == Indication::Reverted {
        MavSeverity::MAV_SEVERITY_WARNING
    } else {
        MavSeverity::MAV_SEVERITY_NOTICE
    };
    MavMessage::STATUSTEXT(STATUSTEXT_DATA {
        severity,
        text: CharArray::new(text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::Endpoint;
    use crate::mavlink_link::{self, MavLink};
    use rta_spec::Verdict;
    use rta_switch::{decide, Request};
    use std::net::UdpSocket;
    use std::time::Duration;

    fn message_id(frame: &[u8]) -> u32 {
        u32::from(frame[7]) | (u32::from(frame[8]) << 8) | (u32::from(frame[9]) << 16)
    }

    #[test]
    fn revert_sends_mode_and_no_setpoint() {
        let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = sink.local_addr().unwrap().port();
        let endpoint = Endpoint::Udp {
            host: "127.0.0.1".into(),
            port,
        };
        let mut link = MavLink::open(&endpoint, 1, 191).unwrap();
        let command = decide(
            Verdict::Revert,
            Some(Request {
                north: 1.0,
                east: 0.0,
                down: 0.0,
                yaw: 0.0,
                commit: true,
            }),
            RecoveryMode::Loiter,
            RecoveryMode::Rtl,
        );
        let message = to_message(&command, 1).unwrap();
        assert!(matches!(message, MavMessage::COMMAND_LONG(_)));
        link.send(message).unwrap();
        let frame = mavlink_link::recv_frame(&sink, Duration::from_millis(1_500)).unwrap();
        assert_eq!(message_id(&frame), 76);
    }

    #[test]
    fn recovered_report_sends_nothing() {
        let command = decide(Verdict::Revert, None, RecoveryMode::Rtl, RecoveryMode::Rtl);
        assert!(to_message(&command, 1).is_none());
        assert_eq!(reported_mode(0x0504_0000), Some(RecoveryMode::Rtl));
    }

    const ALL: [RecoveryMode; 3] = [RecoveryMode::Rtl, RecoveryMode::Loiter, RecoveryMode::Land];

    fn set_mode_params(recovery: RecoveryMode) -> (f32, f32, f32) {
        match to_message(&SwitchCommand::Mode { recovery }, 1) {
            Some(MavMessage::COMMAND_LONG(data)) => {
                assert_eq!(data.command, MavCmd::MAV_CMD_DO_SET_MODE);
                (data.param1, data.param2, data.param3)
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn set_mode_sends_auto_with_the_px4_sub_mode() {
        // PX4 px4_custom_mode.h: main AUTO = 4; sub LOITER = 3, RTL = 5, LAND = 6.
        assert_eq!(set_mode_params(RecoveryMode::Rtl), (1.0, 4.0, 5.0));
        assert_eq!(set_mode_params(RecoveryMode::Loiter), (1.0, 4.0, 3.0));
        assert_eq!(set_mode_params(RecoveryMode::Land), (1.0, 4.0, 6.0));
    }

    #[test]
    fn set_mode_never_requests_offboard_or_acro() {
        for mode in ALL {
            let (_, main, _) = set_mode_params(mode);
            assert_ne!(main, f32::from(PX4_MAIN_MODE_OFFBOARD), "{mode:?}");
            assert_ne!(main, f32::from(PX4_MAIN_MODE_ACRO), "{mode:?}");
        }
    }

    #[test]
    fn encode_decode_round_trip_for_each_recovery_mode() {
        for mode in ALL {
            assert_eq!(reported_mode(custom_mode(mode)), Some(mode));
            let (_, main, sub) = set_mode_params(mode);
            let packed = ((sub as u32) << 24) | ((main as u32) << 16);
            assert_eq!(packed, custom_mode(mode));
            assert_eq!(reported_mode(packed), Some(mode));
        }
    }

    #[test]
    fn real_px4_heartbeat_custom_modes_decode() {
        // HEARTBEAT.custom_mode values as PX4 packs them: sub << 24 | main << 16.
        assert_eq!(reported_mode(0x0504_0000), Some(RecoveryMode::Rtl));
        assert_eq!(reported_mode(0x0304_0000), Some(RecoveryMode::Loiter));
        assert_eq!(reported_mode(0x0604_0000), Some(RecoveryMode::Land));
        assert_eq!(custom_mode(RecoveryMode::Rtl), 0x0504_0000);
        assert_eq!(custom_mode(RecoveryMode::Loiter), 0x0304_0000);
        assert_eq!(custom_mode(RecoveryMode::Land), 0x0604_0000);
        // The reserved low 16 bits do not change the decode.
        assert_eq!(reported_mode(0x0504_ABCD), Some(RecoveryMode::Rtl));
    }

    #[test]
    fn offboard_acro_and_other_auto_modes_are_not_recovery() {
        let not_recovery = [
            0x0006_0000, // OFFBOARD
            0x0005_0000, // ACRO
            0x0003_0000, // POSCTL
            0x0004_0000, // AUTO with no sub-mode
            0x0404_0000, // AUTO_MISSION
            0x0204_0000, // AUTO_TAKEOFF
            0x0104_0000, // AUTO_READY
            0x0506_0000, // OFFBOARD with the RTL sub-mode number
            0x0305_0000, // ACRO with the loiter sub-mode number
            0x0000_0000,
            // The pre-fix encoding: bare 6, 5, and 4 in the low bits.
            6,
            5,
            4,
        ];
        for value in not_recovery {
            assert_eq!(reported_mode(value), None, "{value:#010x}");
        }
    }

    #[test]
    fn set_mode_on_the_wire_carries_auto_and_the_sub_mode() {
        let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = sink.local_addr().unwrap().port();
        let endpoint = Endpoint::Udp {
            host: "127.0.0.1".into(),
            port,
        };
        let mut link = MavLink::open(&endpoint, 1, 191).unwrap();
        let message = to_message(
            &SwitchCommand::Mode {
                recovery: RecoveryMode::Land,
            },
            1,
        )
        .unwrap();
        link.send(message).unwrap();
        let frame = mavlink_link::recv_frame(&sink, Duration::from_millis(1_500)).unwrap();
        assert_eq!(message_id(&frame), 76);
        let mut reader = mavlink::peek_reader::PeekReader::new(frame.as_slice());
        let (_, decoded) = mavlink::read_v2_msg::<MavMessage, _>(&mut reader).unwrap();
        match decoded {
            MavMessage::COMMAND_LONG(data) => {
                assert_eq!(data.command, MavCmd::MAV_CMD_DO_SET_MODE);
                assert_eq!(data.param1, 1.0);
                assert_eq!(data.param2, f32::from(PX4_MAIN_MODE_AUTO));
                assert_eq!(data.param3, f32::from(PX4_SUB_MODE_AUTO_LAND));
            }
            other => panic!("{other:?}"),
        }
    }
}
