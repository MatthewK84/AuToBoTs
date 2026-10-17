//! Reproducible build. The lockfile and the toolchain are the inputs.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

pub fn lock_hash() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    let bytes = fs::read(path).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::lock_hash;

    #[test]
    fn the_index_names_the_toolchain_and_the_lock_hash() {
        let index = include_str!("../../../docs/do178/sci.md");
        let ci = include_str!("../../../.github/workflows/ci.yml");
        let toolchain = include_str!("../../../rust-toolchain.toml");
        assert!(index.contains("1.98.1"));
        assert!(toolchain.contains("1.98.1"));
        assert!(index.contains(&lock_hash()));
        assert!(ci.contains("--locked"));
        assert!(index.contains("floating"));
    }
}
