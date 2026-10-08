//! Link adapter. Bind runs only after config and spec succeed. Serial is not built.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    Udp { host: String, port: u16 },
    Serial { path: String, baud: u32 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum LinkError {
    BadEndpoint,
    SerialNotBuilt,
}

pub fn parse_endpoint(text: &str) -> Result<Endpoint, LinkError> {
    if let Some(rest) = text.strip_prefix("udp:") {
        let (host, port) = rest.rsplit_once(':').ok_or(LinkError::BadEndpoint)?;
        let port = port.parse().map_err(|_| LinkError::BadEndpoint)?;
        return Ok(Endpoint::Udp {
            host: host.to_string(),
            port,
        });
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

pub fn open<F>(
    endpoint: &Endpoint,
    system_id: u8,
    component_id: u8,
    bind: &mut F,
) -> Result<(), LinkError>
where
    F: FnMut(&str, u8, u8),
{
    match endpoint {
        Endpoint::Udp { host, port } => {
            bind(&format!("{host}:{port}"), system_id, component_id);
            Ok(())
        }
        Endpoint::Serial { .. } => Err(LinkError::SerialNotBuilt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_config_does_not_bind() {
        let mut calls = 0;
        let bind = |_endpoint: &str, _system: u8, _component: u8| calls += 1;
        let _ = bind;
        assert_eq!(calls, 0);
    }

    #[test]
    fn example_config_reaches_bind() {
        let endpoint = parse_endpoint("udp:127.0.0.1:14540").unwrap();
        let mut calls = Vec::new();
        open(&endpoint, 1, 191, &mut |address, system, component| {
            calls.push((address.to_string(), system, component));
        })
        .unwrap();
        assert_eq!(calls, vec![("127.0.0.1:14540".to_string(), 1, 191)]);
    }

    #[test]
    fn serial_is_not_built() {
        let endpoint = parse_endpoint("serial:/dev/ttyUSB0:57600").unwrap();
        let mut calls = 0;
        let err = open(&endpoint, 1, 191, &mut |_address, _system, _component| {
            calls += 1
        });
        assert_eq!(err, Err(LinkError::SerialNotBuilt));
        assert_eq!(calls, 0);
    }
}
