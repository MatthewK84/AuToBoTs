//! Link adapter. Bind runs only after config, spec, and log succeed. Serial is not built.
//!
//! `udp:host:port` names the flight controller. The host binds an ephemeral local port, sends
//! to that address, and keeps only datagrams from it. `udpin:host:port` binds that address and
//! replies to the most recent sender, the form a PX4 SITL onboard port expects.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    Udp { host: String, port: u16 },
    UdpIn { host: String, port: u16 },
    Serial { path: String, baud: u32 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum LinkError {
    BadEndpoint,
    SerialNotBuilt,
    NoPeer,
    Io(String),
}

fn host_port(rest: &str) -> Result<(String, u16), LinkError> {
    let (host, port) = rest.rsplit_once(':').ok_or(LinkError::BadEndpoint)?;
    let port = port.parse().map_err(|_| LinkError::BadEndpoint)?;
    if host.is_empty() {
        return Err(LinkError::BadEndpoint);
    }
    Ok((host.to_string(), port))
}

pub fn parse_endpoint(text: &str) -> Result<Endpoint, LinkError> {
    if let Some(rest) = text.strip_prefix("udpin:") {
        let (host, port) = host_port(rest)?;
        return Ok(Endpoint::UdpIn { host, port });
    }
    if let Some(rest) = text.strip_prefix("udp:") {
        let (host, port) = host_port(rest)?;
        return Ok(Endpoint::Udp { host, port });
    }
    if let Some(rest) = text.strip_prefix("serial:") {
        let (path, baud) = rest.rsplit_once(':').ok_or(LinkError::BadEndpoint)?;
        let baud = baud.parse().map_err(|_| LinkError::BadEndpoint)?;
        return Ok(Endpoint::Serial {
            path: path.to_string(),
            baud,
        });
    }
    Err(LinkError::BadEndpoint)
}

fn resolve(host: &str, port: u16) -> Result<SocketAddr, LinkError> {
    (host, port)
        .to_socket_addrs()
        .map_err(|err| LinkError::Io(err.to_string()))?
        .next()
        .ok_or(LinkError::BadEndpoint)
}

fn io(err: std::io::Error) -> LinkError {
    LinkError::Io(err.to_string())
}

/// One bound UDP socket. Clones share the socket and the peer, so the watchdog can send on
/// its own handle without touching the tick thread.
#[derive(Debug)]
pub struct UdpLink {
    socket: UdpSocket,
    peer: Arc<Mutex<Option<SocketAddr>>>,
    fixed_peer: bool,
}

/// Bind the configured endpoint. `read_timeout` bounds every `recv` so a reader can see a
/// stop flag.
pub fn open(endpoint: &Endpoint, read_timeout: Duration) -> Result<UdpLink, LinkError> {
    let (socket, peer, fixed_peer) = match endpoint {
        Endpoint::Udp { host, port } => {
            let remote = resolve(host, *port)?;
            let local: SocketAddr = if remote.is_ipv4() {
                SocketAddr::from(([0, 0, 0, 0], 0))
            } else {
                SocketAddr::from(([0_u16; 8], 0))
            };
            (UdpSocket::bind(local).map_err(io)?, Some(remote), true)
        }
        Endpoint::UdpIn { host, port } => {
            let local = resolve(host, *port)?;
            (UdpSocket::bind(local).map_err(io)?, None, false)
        }
        Endpoint::Serial { .. } => return Err(LinkError::SerialNotBuilt),
    };
    socket
        .set_read_timeout(Some(read_timeout.max(Duration::from_millis(1))))
        .map_err(io)?;
    Ok(UdpLink {
        socket,
        peer: Arc::new(Mutex::new(peer)),
        fixed_peer,
    })
}

impl UdpLink {
    pub fn local_addr(&self) -> Result<SocketAddr, LinkError> {
        self.socket.local_addr().map_err(io)
    }

    pub fn try_clone(&self) -> Result<UdpLink, LinkError> {
        Ok(UdpLink {
            socket: self.socket.try_clone().map_err(io)?,
            peer: Arc::clone(&self.peer),
            fixed_peer: self.fixed_peer,
        })
    }

    pub fn peer(&self) -> Option<SocketAddr> {
        match self.peer.lock() {
            Ok(peer) => *peer,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    fn set_peer(&self, address: SocketAddr) {
        match self.peer.lock() {
            Ok(mut peer) => *peer = Some(address),
            Err(poisoned) => *poisoned.into_inner() = Some(address),
        }
    }

    /// Send one datagram to the peer. `udpin` has no peer until the first datagram arrives.
    pub fn send(&self, bytes: &[u8]) -> Result<(), LinkError> {
        let peer = self.peer().ok_or(LinkError::NoPeer)?;
        self.socket.send_to(bytes, peer).map(|_| ()).map_err(io)
    }

    /// Receive one datagram. `Ok(None)` is a timeout, an ICMP error, or a datagram from a
    /// sender that is not the configured peer. None of those changes an age.
    pub fn recv(&self, buffer: &mut [u8]) -> Result<Option<usize>, LinkError> {
        match self.socket.recv_from(buffer) {
            Ok((count, from)) => {
                if self.fixed_peer {
                    if self.peer() != Some(from) {
                        return Ok(None);
                    }
                } else {
                    self.set_peer(from);
                }
                Ok(Some(count))
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionReset
                ) =>
            {
                Ok(None)
            }
            Err(err) => Err(io(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_millis(500);

    #[test]
    fn bad_config_does_not_bind() {
        for text in [
            "tcp:127.0.0.1:1",
            "udp:127.0.0.1",
            "udp::14540",
            "udpin:x:notaport",
        ] {
            assert_eq!(parse_endpoint(text), Err(LinkError::BadEndpoint), "{text}");
        }
    }

    #[test]
    fn example_config_reaches_bind() {
        let endpoint = parse_endpoint("udp:127.0.0.1:14540").unwrap();
        let link = open(&endpoint, WAIT).unwrap();
        assert_eq!(link.peer(), Some("127.0.0.1:14540".parse().unwrap()));
        assert_ne!(link.local_addr().unwrap().port(), 0);
    }

    #[test]
    fn udp_sends_to_the_peer_and_drops_other_senders() {
        let fc = UdpSocket::bind("127.0.0.1:0").unwrap();
        let stranger = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = fc.local_addr().unwrap().port();
        let link = open(
            &Endpoint::Udp {
                host: "127.0.0.1".into(),
                port,
            },
            WAIT,
        )
        .unwrap();
        link.send(b"hi").unwrap();
        fc.set_read_timeout(Some(WAIT)).unwrap();
        let mut buffer = [0_u8; 16];
        let (count, host) = fc.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..count], b"hi");
        stranger.send_to(b"no", host).unwrap();
        assert_eq!(link.recv(&mut buffer), Ok(None));
        fc.send_to(b"ok", host).unwrap();
        assert_eq!(link.recv(&mut buffer), Ok(Some(2)));
        assert_eq!(&buffer[..2], b"ok");
    }

    #[test]
    fn udpin_binds_the_endpoint_and_replies_to_the_last_sender() {
        let link = open(
            &Endpoint::UdpIn {
                host: "127.0.0.1".into(),
                port: 0,
            },
            WAIT,
        )
        .unwrap();
        assert_eq!(link.send(b"x"), Err(LinkError::NoPeer));
        let bound = link.local_addr().unwrap();
        let fc = UdpSocket::bind("127.0.0.1:0").unwrap();
        fc.set_read_timeout(Some(WAIT)).unwrap();
        fc.send_to(b"hb", bound).unwrap();
        let mut buffer = [0_u8; 16];
        assert_eq!(link.recv(&mut buffer), Ok(Some(2)));
        let clone = link.try_clone().unwrap();
        clone.send(b"back").unwrap();
        let (count, _) = fc.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..count], b"back");
    }

    #[test]
    fn serial_is_not_built() {
        let endpoint = parse_endpoint("serial:/dev/ttyUSB0:57600").unwrap();
        assert_eq!(
            open(&endpoint, WAIT).map(|_| ()),
            Err(LinkError::SerialNotBuilt)
        );
    }
}
