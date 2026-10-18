//! Document and source-text checks for `rta-host/src/eval_worker.rs`.

#[test]
fn the_deadline_note_names_the_handoff_the_latch_and_the_cost() {
    let note = include_str!("../../../../docs/deadline.md");
    for item in [
        "capacity one",
        "latches like an interpreter error",
        "discarded by sequence number",
        "shutdown detaches it",
        "No queue grows and no thread is spawned per tick.",
        "instead of one tick",
    ] {
        assert!(note.contains(item), "{item}");
    }
    let requirements = include_str!("../../../../docs/requirements.md");
    assert!(requirements.contains("whether or not eval returns, and latch revert"));
    let source = include_str!("../../../rta-host/src/eval_worker.rs");
    assert!(source.contains("sync_channel::<Request>(1)"));
    assert!(source.contains("sync_channel::<Response>(1)"));
}
