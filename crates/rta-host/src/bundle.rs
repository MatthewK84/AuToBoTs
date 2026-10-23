//! Reload bundle. Rejection does not replace the running monitor.

use crate::spec::load_spec;
use crate::tick_log::spec_hash;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningSpec {
    pub text: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    pub spec: String,
    pub weights: Option<Vec<u8>>,
    pub previous_hash: String,
    pub new_hash: String,
    pub notes_hash: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleError {
    Unsigned,
    Unparsable,
    HashMismatch,
}

pub fn spec_pin(text: &str) -> String {
    format!("{:016x}", spec_hash(text.as_bytes()))
}

pub fn manifest_bytes(previous_hash: &str, new_hash: &str) -> String {
    format!("previous_hash={previous_hash}\nnew_hash={new_hash}\n")
}

/// Detached pin of the manifest. This is not a qualified cryptographic signature.
pub fn sign_manifest(previous_hash: &str, new_hash: &str) -> String {
    format!(
        "{:016x}",
        spec_hash(manifest_bytes(previous_hash, new_hash).as_bytes())
    )
}

/// Accept a bundle or refuse it. The running spec is borrowed and is not replaced here.
pub fn accept(running: &RunningSpec, bundle: &Bundle) -> Result<RunningSpec, BundleError> {
    let Some(signature) = bundle.signature.as_deref() else {
        return Err(BundleError::Unsigned);
    };
    if signature.is_empty() || signature != sign_manifest(&bundle.previous_hash, &bundle.new_hash) {
        return Err(BundleError::Unsigned);
    }
    if load_spec(&bundle.spec).is_err() {
        return Err(BundleError::Unparsable);
    }
    if spec_pin(&bundle.spec) != bundle.new_hash || bundle.previous_hash != running.hash {
        return Err(BundleError::HashMismatch);
    }
    Ok(RunningSpec {
        text: bundle.spec.clone(),
        hash: bundle.new_hash.clone(),
    })
}

pub fn load_dir(running: &RunningSpec, dir: &Path) -> Result<RunningSpec, BundleError> {
    let spec = fs::read_to_string(dir.join("monitor.lola")).unwrap_or_default();
    let manifest = fs::read_to_string(dir.join("manifest")).unwrap_or_default();
    let signature = fs::read_to_string(dir.join("manifest.sig")).ok();
    let weights = fs::read(dir.join("weights")).ok();
    let mut previous_hash = String::new();
    let mut new_hash = String::new();
    for line in manifest.lines() {
        if let Some(value) = line.strip_prefix("previous_hash=") {
            previous_hash = value.to_string();
        } else if let Some(value) = line.strip_prefix("new_hash=") {
            new_hash = value.to_string();
        }
    }
    accept(
        running,
        &Bundle {
            spec,
            weights,
            previous_hash,
            new_hash,
            notes_hash: String::new(),
            signature: signature.map(|text| text.trim().to_string()),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running() -> RunningSpec {
        let text = include_str!("../../../spec/monitor.lola").to_string();
        let hash = spec_pin(&text);
        RunningSpec { text, hash }
    }

    fn signed(running: &RunningSpec, spec: &str) -> Bundle {
        let new_hash = spec_pin(spec);
        Bundle {
            spec: spec.to_string(),
            weights: None,
            previous_hash: running.hash.clone(),
            new_hash: new_hash.clone(),
            notes_hash: String::new(),
            signature: Some(sign_manifest(&running.hash, &new_hash)),
        }
    }

    #[test]
    // trace: H11
    fn unsigned_is_rejected_and_the_monitor_stays() {
        let running = running();
        let before = running.clone();
        let mut bundle = signed(&running, &running.text);
        bundle.signature = None;
        assert_eq!(accept(&running, &bundle), Err(BundleError::Unsigned));
        bundle.signature = Some(String::new());
        assert_eq!(accept(&running, &bundle), Err(BundleError::Unsigned));
        assert_eq!(running, before);
    }

    #[test]
    fn unparsable_is_rejected_and_the_monitor_stays() {
        let running = running();
        let before = running.clone();
        let bundle = signed(&running, "this is not a spec");
        assert_eq!(accept(&running, &bundle), Err(BundleError::Unparsable));
        assert_eq!(running, before);
    }

    #[test]
    fn hash_mismatch_is_rejected_and_the_monitor_stays() {
        let running = running();
        let before = running.clone();
        let mut bundle = signed(&running, &running.text);
        bundle.new_hash = "0000000000000000".into();
        bundle.signature = Some(sign_manifest(&bundle.previous_hash, &bundle.new_hash));
        assert_eq!(accept(&running, &bundle), Err(BundleError::HashMismatch));
        bundle = signed(&running, &running.text);
        bundle.previous_hash = "1111111111111111".into();
        bundle.signature = Some(sign_manifest(&bundle.previous_hash, &bundle.new_hash));
        assert_eq!(accept(&running, &bundle), Err(BundleError::HashMismatch));
        assert_eq!(running, before);
    }

    #[test]
    fn a_directory_bundle_rejects_a_missing_signature() {
        let running = running();
        let dir = std::env::temp_dir().join(format!("rta-bundle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("monitor.lola"), &running.text).unwrap();
        fs::write(
            dir.join("manifest"),
            manifest_bytes(&running.hash, &running.hash),
        )
        .unwrap();
        let err = load_dir(&running, &dir).expect_err("unsigned");
        assert_eq!(err, BundleError::Unsigned);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_signed_parsable_match_is_accepted_without_replacing_the_caller() {
        let running = running();
        let before = running.clone();
        let bundle = signed(&running, &running.text);
        let next = accept(&running, &bundle).expect("accepted");
        assert_eq!(next.hash, running.hash);
        assert_eq!(running, before);
    }
}
