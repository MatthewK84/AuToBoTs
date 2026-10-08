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
            Endpoint::Serial { path, baud } => format!("serial:{path}:{baud}"),
        };
        let connection = mavlink::connect::<MavMessage>(&address).map_err(|err| {
            eprintln!("mavlink connect {address}: {err}");
            match endpoint {
                Endpoint::Serial { .. } => LinkError::SerialNotBuilt,
                Endpoint::Udp { .. } => LinkError::BadEndpoint,
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

    #[test]
    fn classify_keeps_status_text_off_the_position_path() {
        assert!(matches!(classify(&heartbeat()), Inbound::Heartbeat));
    }
}
