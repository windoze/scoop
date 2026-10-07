use super::{compile_and_run, workspace_root};

#[test]
fn full_only_diagnostic_keeps_the_same_nursery_allocation_path() {
    let output = compile_and_run(
        &workspace_root(),
        "full_only_test",
        "runtime/tests/full_only_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"full-only collection preserves nursery allocation and roots\n"
    );
}

#[test]
fn minor_rewrites_callback_native_and_frozen_segment_roots() {
    let output = compile_and_run(
        &workspace_root(),
        "nursery_thread_protocol_test",
        "runtime/tests/nursery_thread_protocol_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"moving collector thread-protocol tests passed\n"
    );
}

#[test]
fn concurrent_mutators_mark_the_same_old_card_during_minor_collection() {
    let output = compile_and_run(
        &workspace_root(),
        "nursery_mutators_test",
        "runtime/tests/nursery_mutators_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"concurrent nursery mutators preserve writes to the same old card\n"
    );
}

#[test]
fn minor_traces_dirty_ranges_and_promotes_pinned_blocks() {
    let output = compile_and_run(
        &workspace_root(),
        "nursery_test",
        "runtime/tests/nursery_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"nursery graphs, dirty array ranges, pin, handle and release passed\n"
    );
}

#[test]
fn failed_promotion_reservation_falls_back_without_changing_the_graph() {
    let output = compile_and_run(
        &workspace_root(),
        "nursery_fallback_test",
        "runtime/tests/nursery_fallback_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"partial promotion reservation rolls back before full collection\n"
    );
}
