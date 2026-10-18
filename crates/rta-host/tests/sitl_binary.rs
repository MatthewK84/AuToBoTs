//! The `rta-sitl` binary writes the hash of the spec it loaded, not zero.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn rta_sitl_logs_the_real_spec_hash() {
    let dir = std::env::temp_dir().join(format!("rta-sitl-bin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec_path = root.join("spec/monitor.lola");
    let spec = std::fs::read(&spec_path).unwrap();
    let log = dir.join("sitl.log");
    let config = include_str!("../../../config/sitl.toml")
        .replace("spec/monitor.lola", spec_path.to_str().unwrap())
        .replace("\"sitl.log\"", &format!("{:?}", log.to_str().unwrap()));
    let config_path = dir.join("sitl.toml");
    std::fs::write(&config_path, config).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rta-sitl"))
        .arg(&config_path)
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = std::fs::read_to_string(&log).unwrap();
    let expected = format!("spec_hash={:016x}", rta_host::tick_log::spec_hash(&spec));
    assert!(!text.contains("spec_hash=0000000000000000"), "{text}");
    let tick = text.lines().find(|l| l.starts_with("tick=")).unwrap();
    assert!(tick.contains(&expected), "{tick}");
    for header in text.lines().filter(|l| l.starts_with("# rta-log")) {
        assert!(header.contains(&expected), "{header}");
    }
    assert!(
        tick.contains("verdict=pass") && tick.contains("fence_ok=1"),
        "{tick}"
    );
    let timing = std::fs::read_to_string(dir.join("sitl-timing.txt")).unwrap();
    assert!(timing.contains(&expected), "{timing}");
    let _ = std::fs::remove_dir_all(&dir);
}
