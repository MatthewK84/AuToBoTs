//! Authorizing-official packet. An index, not a signature.

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn index_links_resolve_and_gaps_stay_listed() {
        let packet = include_str!("../../../docs/do178/ao-packet.md");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/do178");
        for line in packet.lines() {
            if let Some(start) = line.find("](") {
                if let Some(end) = line[start + 2..].find(')') {
                    let link = &line[start + 2..start + 2 + end];
                    if link.starts_with("http") {
                        continue;
                    }
                    let path = root.join(link);
                    assert!(path.is_file(), "{link}");
                }
            }
        }
        assert!(packet.contains("M11.3"));
        assert!(packet.contains("M12.2"));
        assert!(packet.contains("M13"));
        assert!(packet.contains("M16.2"));
        assert!(packet.contains("not folded into a compliance sentence"));
        assert!(packet.contains("Recovery is the flight controller"));
        assert!(packet.contains("complex function is not pedigreed"));
        assert!(packet.contains("not a signature block"));
        assert!(!packet.contains("signed by"));
    }
}
