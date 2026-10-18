//! Document and source-text checks for `rta-host/src/weights.rs`.

#[test]
fn the_shadow_note_says_it_is_off_and_compiled_out() {
    let note = include_str!("../../../../docs/shadow-net.md");
    assert!(note.contains("SHADOW_DEFAULT_ON"));
    assert!(note.contains("compiled out"));
    assert!(note.contains("not raised"));
    assert!(note.contains("Group 1"));
    assert!(note.contains("unmeasured"));
}
