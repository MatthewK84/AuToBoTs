//! Read path. Age uses local receive time. A status text does not change the record.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartialRecord {
    pub alt_m: Option<f64>,
    pub fix_received_ms: Option<u64>,
    pub heartbeat_received_ms: Option<u64>,
    pub track_conf: Option<f64>,
    pub range_m: Option<f64>,
    pub status_log: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    Unknown,
}

pub fn apply(bytes: &[u8], now_ms: u64, record: &mut PartialRecord) -> Result<(), ReadError> {
    match bytes.first().copied() {
        Some(1) if bytes.len() == 5 => {
            let alt_mm = i32::from_le_bytes(bytes[1..5].try_into().unwrap());
            record.alt_m = Some(alt_mm as f64 / 1_000.0);
            record.fix_received_ms = Some(now_ms);
            Ok(())
        }
        Some(2) => {
            record.heartbeat_received_ms = Some(now_ms);
            Ok(())
        }
        Some(3) => {
            record
                .status_log
                .push(String::from_utf8_lossy(&bytes[1..]).to_string());
            Ok(())
        }
        _ => Err(ReadError::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_blob_updates_altitude_and_fix_time_only() {
        let mut record = PartialRecord {
            track_conf: Some(0.8),
            ..PartialRecord::default()
        };
        let mut bytes = vec![1];
        bytes.extend(1_500_i32.to_le_bytes());
        apply(&bytes, 40, &mut record).unwrap();
        assert_eq!(record.alt_m, Some(1.5));
        assert_eq!(record.fix_received_ms, Some(40));
        assert_eq!(record.track_conf, Some(0.8));
        assert_eq!(record.heartbeat_received_ms, None);
    }

    #[test]
    fn status_text_does_not_change_the_record() {
        let mut record = PartialRecord {
            alt_m: Some(2.0),
            ..PartialRecord::default()
        };
        let mut bytes = vec![3];
        bytes.extend(b"gps fail");
        apply(&bytes, 80, &mut record).unwrap();
        assert_eq!(record.alt_m, Some(2.0));
        assert_eq!(record.fix_received_ms, None);
        assert_eq!(record.status_log, vec!["gps fail".to_string()]);
    }
}
