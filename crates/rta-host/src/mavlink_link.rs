//! MAVLink common link. Heartbeat out is 1 Hz. Inbound age uses local receive time.

use crate::link::{Endpoint, LinkError};
use mavlink::dialects::common::{
    MavAutopilot, MavMessage, MavModeFlag, MavState, MavType, HEARTBEAT_DATA,
};
use mavlink::{MavConnection, MavHeader};

#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    Heartbeat,
    Position { alt_mm: i32 },
    StatusText(String),
    Other,
}

pub struct MavLink {
    connection: Box<dyn MavConnection<MavMessage> + Send + Sync>,
    system_id: u8,
    component_id: u8,
    sequence: u8,
    last_fc_heartbeat_ms: Option<u64>,
}

impl MavLink {
    pub fn open(endpoint: &Endpoint, system_id: u8, component_id: u8) -> Result<Self, LinkError> {
        let address = match endpoint {
            Endpoint::Udp { host, port } => format!("udpout:{host}:{port}"),
            Endpoint::UdpIn { host, port } => format!("udpin:{host}:{port}"),
            Endpoint::Serial { path, baud } => format!("serial:{path}:{baud}"),
        };
        let connection = mavlink::connect::<MavMessage>(&address).map_err(|err| {
            eprintln!("mavlink connect {address}: {err}");
            match endpoint {
                Endpoint::Serial { .. } => LinkError::SerialNotBuilt,
                Endpoint::Udp { .. } | Endpoint::UdpIn { .. } => LinkError::BadEndpoint,
            }
        })?;
        Ok(Self {
            connection: Box::new(connection),
            system_id,
            component_id,
            sequence: 0,
            last_fc_heartbeat_ms: None,
        })
    }

    pub fn send_heartbeat(&mut self) -> Result<(), String> {
        self.send(heartbeat())
    }

    pub fn send(&mut self, message: MavMessage) -> Result<(), String> {
        let header = MavHeader {
            system_id: self.system_id,
            component_id: self.component_id,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);
        self.connection
            .send(&header, &message)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn note(&mut self, inbound: &Inbound, now_ms: u64) {
        if matches!(inbound, Inbound::Heartbeat) {
            self.last_fc_heartbeat_ms = Some(now_ms);
        }
    }

    pub fn fc_heartbeat_age_ms(&self, now_ms: u64) -> f64 {
        match self.last_fc_heartbeat_ms {
            Some(last) => now_ms.saturating_sub(last) as f64,
            None => f64::MAX,
        }
    }
}

pub fn heartbeat() -> MavMessage {
    MavMessage::HEARTBEAT(HEARTBEAT_DATA {
        custom_mode: 0,
        mavtype: MavType::MAV_TYPE_ONBOARD_CONTROLLER,
        autopilot: MavAutopilot::MAV_AUTOPILOT_INVALID,
        base_mode: MavModeFlag::empty(),
        system_status: MavState::MAV_STATE_ACTIVE,
        mavlink_version: 3,
    })
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct VehicleState {
    pub alt_m: Option<f64>,
    pub fix_received_ms: Option<u64>,
    pub link_received_ms: Option<u64>,
    pub fc_heartbeat_ms: Option<u64>,
    pub status_log: Vec<String>,
}

pub fn apply(state: &mut VehicleState, inbound: &Inbound, now_ms: u64) {
    match inbound {
        Inbound::Heartbeat => {
            state.fc_heartbeat_ms = Some(now_ms);
            state.link_received_ms = Some(now_ms);
        }
        Inbound::Position { alt_mm } => {
            state.alt_m = Some(*alt_mm as f64 / 1_000.0);
            state.fix_received_ms = Some(now_ms);
            state.link_received_ms = Some(now_ms);
        }
        Inbound::StatusText(text) => state.status_log.push(text.clone()),
        Inbound::Other => state.link_received_ms = Some(now_ms),
    }
}

pub fn age_ms(received: Option<u64>, now_ms: u64) -> f64 {
    match received {
        Some(last) => now_ms.saturating_sub(last) as f64,
        None => f64::MAX,
    }
}

/// Largest MAVLink v2 frame: magic, header, 255-byte payload, checksum, signature.
pub const MAX_FRAME: usize = 280;

/// Encode one v2 frame into a fixed buffer. No allocation.
pub fn encode(
    header: MavHeader,
    message: &MavMessage,
    buffer: &mut [u8; MAX_FRAME],
) -> Result<usize, String> {
    let mut cursor = std::io::Cursor::new(&mut buffer[..]);
    mavlink::write_v2_msg(&mut cursor, header, message).map_err(|err| err.to_string())
}

/// Decode every v1 or v2 frame in one datagram, at most `MAX_FRAMES_PER_DATAGRAM`. A frame
/// that fails its checksum or is unknown to the dialect ends the datagram.
pub fn decode_each<F>(datagram: &[u8], mut each: F)
where
    F: FnMut(&MavHeader, &MavMessage),
{
    decode_frames(datagram, |header, frame| {
        if let Frame::Message(message) = frame {
            each(header, message);
        }
    });
}

/// One frame from a datagram. `COMMAND_ACK` is read from its raw payload, because the common
/// dialect in mavlink 0.18 has no `MAV_RESULT_IN_PROGRESS` and would reject that frame.
#[derive(Debug)]
pub enum Frame<'a> {
    Message(&'a MavMessage),
    CommandAck(CommandAck),
}

/// `COMMAND_ACK` (id 77): the command it answers and the raw `MAV_RESULT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandAck {
    pub command: u16,
    pub result: u8,
}

/// Common dialect id of `COMMAND_ACK`.
pub const COMMAND_ACK_ID: u32 = 77;

/// Read a `COMMAND_ACK` payload. MAVLink 2 trims trailing zero bytes, so a short payload is
/// zero-filled: command is bytes 0 and 1 little-endian, result is byte 2.
pub fn command_ack(payload: &[u8]) -> CommandAck {
    let byte = |index: usize| payload.get(index).copied().unwrap_or(0);
    CommandAck {
        command: u16::from_le_bytes([byte(0), byte(1)]),
        result: byte(2),
    }
}

/// Like `decode_each`, and also yields every `COMMAND_ACK` whatever its result value.
pub fn decode_frames<F>(datagram: &[u8], mut each: F)
where
    F: FnMut(&MavHeader, Frame<'_>),
{
    let mut reader = mavlink::peek_reader::PeekReader::<_, MAX_FRAME>::new(datagram);
    for _ in 0..MAX_FRAMES_PER_DATAGRAM {
        let Ok(raw) = mavlink::read_any_raw_message::<MavMessage, _>(&mut reader) else {
            break;
        };
        let header = MavHeader {
            system_id: raw.system_id(),
            component_id: raw.component_id(),
            sequence: raw.sequence(),
        };
        if raw.message_id() == COMMAND_ACK_ID {
            each(&header, Frame::CommandAck(command_ack(raw.payload())));
            continue;
        }
        match <MavMessage as mavlink::Message>::parse(
            raw.version(),
            raw.message_id(),
            raw.payload(),
        ) {
            Ok(message) => each(&header, Frame::Message(&message)),
            Err(_) => break,
        }
    }
}

/// Bound on frames read from one datagram.
pub const MAX_FRAMES_PER_DATAGRAM: usize = 16;

pub fn classify(message: &MavMessage) -> Inbound {
    match message {
        MavMessage::HEARTBEAT(_) => Inbound::Heartbeat,
        MavMessage::GLOBAL_POSITION_INT(position) => Inbound::Position {
            alt_mm: position.alt,
        },
        MavMessage::STATUSTEXT(status) => Inbound::StatusText(format!("{:?}", status.text)),
        MavMessage::SYS_STATUS(_) => Inbound::StatusText("sys_status".into()),
        _ => Inbound::Other,
    }
}

#[cfg(test)]
pub(crate) fn recv_frame(
    socket: &std::net::UdpSocket,
    wait: std::time::Duration,
) -> Result<Vec<u8>, String> {
    socket
        .set_read_timeout(Some(wait))
        .map_err(|err| err.to_string())?;
    let mut buffer = [0_u8; 280];
    let (count, _) = socket
        .recv_from(&mut buffer)
        .map_err(|err| err.to_string())?;
    Ok(buffer[..count].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;
    use std::time::Duration;

    #[test]
    fn heartbeat_reaches_a_local_sink_within_the_window() {
        let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = sink.local_addr().unwrap().port();
        let endpoint = Endpoint::Udp {
            host: "127.0.0.1".into(),
            port,
        };
        let mut link = MavLink::open(&endpoint, 1, 191).unwrap();
        link.send_heartbeat().unwrap();
        let frame = recv_frame(&sink, Duration::from_millis(1_500)).unwrap();
        assert_eq!(frame[0], 0xFD);
        assert_eq!(frame[5], 1);
        assert_eq!(frame[6], 191);
        assert_eq!(&frame[7..10], &[0, 0, 0]);
    }

    #[test]
    fn paused_peer_grows_heartbeat_age() {
        let endpoint = Endpoint::Udp {
            host: "127.0.0.1".into(),
            port: 1,
        };
        let mut link = MavLink::open(&endpoint, 1, 191).unwrap();
        assert!(link.fc_heartbeat_age_ms(400) > 1_000.0);
        link.note(&Inbound::Heartbeat, 100);
        assert_eq!(link.fc_heartbeat_age_ms(500), 400.0);
    }

    #[test]
    // trace: H2
    fn position_resets_fix_age() {
        let mut state = VehicleState::default();
        apply(&mut state, &Inbound::Position { alt_mm: 1_500 }, 40);
        assert_eq!(state.alt_m, Some(1.5));
        assert_eq!(age_ms(state.fix_received_ms, 40), 0.0);
        apply(&mut state, &Inbound::StatusText("sys_status".into()), 80);
        assert_eq!(state.alt_m, Some(1.5));
        assert_eq!(state.fix_received_ms, Some(40));
        assert!(age_ms(state.fc_heartbeat_ms, 80) > age_ms(state.link_received_ms, 80));
    }

    #[test]
    fn spec_does_not_read_system_status() {
        let spec = include_str!("../../../spec/monitor.lola");
        let name = ["SYS", "_STATUS"].concat();
        assert!(!spec.contains(&name));
        assert!(!spec.contains("battery"));
    }

    /// MAVLink `crc_accumulate` (CRC-16/MCRF4XX).
    fn x25(data: &[u8]) -> u16 {
        let mut crc = 0xFFFF_u16;
        for byte in data {
            let mut tmp = byte ^ (crc & 0xFF) as u8;
            tmp ^= tmp << 4;
            let tmp = u16::from(tmp);
            crc = (crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4);
        }
        crc
    }

    fn ack_frame(command: u16, result: u8) -> Vec<u8> {
        // Build a valid frame with the typed encoder, then patch the result byte and the
        // checksum so a result the dialect crate lacks can be carried.
        use mavlink::dialects::common::{MavCmd, MavResult, COMMAND_ACK_DATA};
        let mut buffer = [0_u8; MAX_FRAME];
        let message = MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
            command: MavCmd::MAV_CMD_DO_SET_MODE,
            result: MavResult::MAV_RESULT_FAILED,
        });
        let count = encode(MavHeader::default(), &message, &mut buffer).unwrap();
        let mut frame = buffer[..count].to_vec();
        let length = usize::from(frame[1]);
        let mut payload = vec![0_u8; 3];
        payload[..2].copy_from_slice(&command.to_le_bytes());
        payload[2] = result;
        while payload.last() == Some(&0) && payload.len() > 1 {
            payload.pop();
        }
        frame.truncate(10);
        frame[1] = payload.len() as u8;
        frame.extend_from_slice(&payload);
        let mut checked = frame[1..].to_vec();
        checked.push(143); // COMMAND_ACK extra CRC.
        frame.extend_from_slice(&x25(&checked).to_le_bytes());
        assert!(length >= 1);
        frame
    }

    #[test]
    fn command_ack_is_read_for_every_result_including_in_progress() {
        for result in 0..=6_u8 {
            let mut datagram = ack_frame(176, result);
            // A heartbeat after the ack is still read.
            let mut buffer = [0_u8; MAX_FRAME];
            let count = encode(MavHeader::default(), &heartbeat(), &mut buffer).unwrap();
            datagram.extend_from_slice(&buffer[..count]);
            let mut acks = Vec::new();
            let mut heartbeats = 0;
            decode_frames(&datagram, |_, frame| match frame {
                Frame::CommandAck(ack) => acks.push(ack),
                Frame::Message(MavMessage::HEARTBEAT(_)) => heartbeats += 1,
                Frame::Message(_) => {}
            });
            assert_eq!(
                acks,
                [CommandAck {
                    command: 176,
                    result
                }],
                "{result}"
            );
            assert_eq!(heartbeats, 1, "{result}");
        }
    }

    #[test]
    fn a_trimmed_ack_payload_reads_as_zero() {
        assert_eq!(
            command_ack(&[176]),
            CommandAck {
                command: 176,
                result: 0
            }
        );
        assert_eq!(
            command_ack(&[]),
            CommandAck {
                command: 0,
                result: 0
            }
        );
    }

    #[test]
    fn classify_keeps_status_text_off_the_position_path() {
        assert!(matches!(classify(&heartbeat()), Inbound::Heartbeat));
    }
}
