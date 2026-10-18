//! The `rta-host` binary end to end: config, spec, log, bind, loop, shutdown.

use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rta-host-bin-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_config(dir: &Path, endpoint: &str) -> PathBuf {
    let spec = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/monitor.lola");
    let text = include_str!("../../../config/example.toml")
        .replace("udp:127.0.0.1:14540", endpoint)
        .replace("spec/monitor.lola", spec.to_str().unwrap())
        .replace("rta.log", dir.join("rta.log").to_str().unwrap());
    let path = dir.join("host.toml");
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn bounded_run_binds_heartbeats_and_writes_the_log() {
    let dir = scratch("bounded");
    let fc = UdpSocket::bind("127.0.0.1:0").unwrap();
    fc.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let endpoint = format!("udp:127.0.0.1:{}", fc.local_addr().unwrap().port());
    let config = write_config(&dir, &endpoint);
    let output = Command::new(env!("CARGO_BIN_EXE_rta-host"))
        .arg(&config)
        .args(["--ticks", "4"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("stopped after 4 ticks"), "{stdout}");
    let mut buffer = [0_u8; 280];
    let (count, _) = fc.recv_from(&mut buffer).unwrap();
    // MAVLink v2 HEARTBEAT from system 1, component 191.
    assert_eq!(buffer[0], 0xFD);
    assert_eq!(&buffer[5..10], &[1, 191, 0, 0, 0]);
    assert!(count > 10);
    let log = std::fs::read_to_string(dir.join("rta.log")).unwrap();
    assert_eq!(
        log.lines().filter(|line| line.starts_with("tick=")).count(),
        4
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_config_exits_before_a_socket_and_writes_no_log() {
    let dir = scratch("bad");
    let config = write_config(&dir, "tcp:127.0.0.1:1");
    let output = Command::new(env!("CARGO_BIN_EXE_rta-host"))
        .arg(&config)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("config:"));
    assert!(!dir.join("rta.log").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sigint_stops_the_loop_and_flushes_the_log() {
    let dir = scratch("sigint");
    let fc = UdpSocket::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("udp:127.0.0.1:{}", fc.local_addr().unwrap().port());
    let config = write_config(&dir, &endpoint);
    let child = Command::new(env!("CARGO_BIN_EXE_rta-host"))
        .arg(&config)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let log = dir.join("rta.log");
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let ticks = std::fs::read_to_string(&log)
            .map(|text| {
                text.lines()
                    .filter(|line| line.starts_with("tick="))
                    .count()
            })
            .unwrap_or(0);
        if ticks >= 3 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let killed = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("rta-host: stopped after"));
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(
        text.lines()
            .filter(|line| line.starts_with("tick="))
            .count()
            >= 3
    );
    let _ = std::fs::remove_dir_all(&dir);
}
