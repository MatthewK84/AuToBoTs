//! Software configuration index. Hashes, not branch tips.

use sha2::{Digest, Sha256};

/// Recorded spec hash against the file on disk. Drift fails CI.
pub fn spec_hash_matches(index: &str, spec: &str) -> bool {
    hash_matches(index, spec.as_bytes())
}

pub fn hash_matches(index: &str, bytes: &[u8]) -> bool {
    let digest = Sha256::digest(bytes);
    let hex = format!("{digest:x}");
    index.contains(&hex)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    #[test]
    fn the_index_names_hashes_and_the_swap_updates_it() {
        let index = include_str!("../../../docs/do178/sci.md");
        let bundle = include_str!("../../../docs/reload-bundle.md");
        assert!(index.contains("ae398e85f30cd1e8723bbef36ad934b3651017e6d987eeedbc7fddf229bd6553"));
        assert!(index.contains("747ff29188a50757aed426688ee7229695d09a62bdd8a947845bc67c3d2781db"));
        assert!(index.contains("8ef36e2c13b88ae726738d52da06723be20e7ad52127db46f0b8b069dfb052ac"));
        assert!(index.contains("not floating branch tips"));
        assert!(!index.contains("branch: main"));
        assert!(index.contains("new index row, not an edit"));
        assert!(index.contains("same change as the swap"));
        assert!(bundle.contains("same change as the swap"));
        assert!(index.contains("rtlola-interpreter"));
        assert!(index.contains("0.11.0"));
    }

    #[test]
    fn changed_config_without_an_index_update_fails() {
        let index = include_str!("../../../docs/do178/sci.md");
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/example.toml");
        let bytes = fs::read(&path).unwrap();
        assert!(super::hash_matches(index, &bytes));
        assert!(!super::hash_matches(index, b"tampered config"));
        assert!(index.contains("named file with its own hash"));
        assert!(
            index.contains("Not a flight file")
                || std::str::from_utf8(&bytes)
                    .unwrap()
                    .contains("Not a flight file")
        );
    }

    #[test]
    fn tampering_with_the_spec_without_updating_the_index_fails() {
        let index = include_str!("../../../docs/do178/sci.md");
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/monitor.lola");
        let spec = fs::read_to_string(path).unwrap();
        assert!(super::spec_hash_matches(index, &spec));
        assert!(!super::spec_hash_matches(index, "tampered"));
        let packet = include_str!("../../../docs/do178/ao-packet.md");
        assert!(packet.contains("sci.md"));
    }
}
