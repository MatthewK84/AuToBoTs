//! Document and source-text checks for `rta-host/src/tracker.rs`.

#[test]
fn crate_has_no_video_library() {
    let manifest = include_str!("../../../rta-host/Cargo.toml");
    let video = ["g", "streamer"].concat();
    assert!(!manifest.contains(&video));
    assert!(!manifest.contains("opencv"));
    assert!(!manifest.contains("ffmpeg"));
}

#[test]
fn the_tracker_port_names_no_frame_and_no_decision() {
    let contract = include_str!("../../../rta-host/src/tracker.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("contract");
    assert!(!contract.contains("appsink"));
    assert!(!contract.contains("Buffer"));
    assert!(!contract.contains("decide"));
}
