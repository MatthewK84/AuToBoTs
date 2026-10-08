//! Outbound commands. Mode and setpoints are separate messages. Idle sends nothing.

use mavlink::dialects::common::{
    MavCmd, MavMessage, PositionTargetTypemask, COMMAND_LONG_DATA,
    SET_POSITION_TARGET_LOCAL_NED_DATA,
};
use rta_spec::RecoveryMode;
use rta_switch::SwitchCommand;

pub fn to_message(command: &SwitchCommand, target_system: u8) -> Option<MavMessage> {
    match command {
        SwitchCommand::Mode { recovery } => Some(MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
            param1: 1.0,
            param2: custom_mode(*recovery) as f32,
            param3: 0.0,
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

pub fn custom_mode(mode: RecoveryMode) -> u32 {
    match mode {
        RecoveryMode::Rtl => 6,
        RecoveryMode::Loiter => 5,
        RecoveryMode::Land => 4,
    }
}

pub fn reported_mode(custom_mode: u32) -> Option<RecoveryMode> {
    match custom_mode {
        6 => Some(RecoveryMode::Rtl),
        5 => Some(RecoveryMode::Loiter),
        4 => Some(RecoveryMode::Land),
        _ => None,
    }
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
        assert_eq!(reported_mode(6), Some(RecoveryMode::Rtl));
    }
}
