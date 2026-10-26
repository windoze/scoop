use super::{compile_and_run, compile_and_run_with_flags, workspace_root};

#[test]
fn tlab_reserves_preserve_capacity_failures_and_retirement() {
    for optimization in ["-O0", "-O2"] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "tlab_reserve_test",
            "runtime/tests/tlab_reserve_test.c",
            true,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"TLAB reserves preserve capacity, partial failures, moving roots and retirement\n"
        );
    }
}

#[test]
fn dense_promotion_preserves_sparse_relocation_reuse_and_old_writes() {
    for flags in [vec!["-O0"], vec!["-O2", "-DSCOOP_TEST_PROMOTION_PARALLEL"]] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "dense_promotion_test",
            "runtime/tests/dense_promotion_test.c",
            true,
            &flags,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"dense promotion preserves aliases, sparse relocation, reuse and old writes\n"
        );
    }
}

#[test]
fn array_scans_preserve_exact_ranges_order_and_metadata() {
    for optimization in ["-O0", "-O2"] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "scan_ranges_test",
            "runtime/tests/scan_ranges_test.c",
            false,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"flat and recursive array scans preserve exact slot ranges, order and metadata\n"
        );
    }
}

#[test]
fn survivor_lifetimes_and_remembered_edges_are_preserved() {
    for flags in [vec!["-O2"], vec!["-O2", "-DSCOOP_TEST_SURVIVOR_PARALLEL"]] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "survivor_test",
            "runtime/tests/survivor_test.c",
            true,
            &flags,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"survivor lifetimes, remembered edges, ages, pinning and counts are preserved\n"
        );
    }
}

#[test]
fn adaptive_nursery_follows_graph_lifetimes_and_preserves_old_pressure() {
    for flags in [vec!["-O2"], vec!["-O2", "-DSCOOP_TEST_NURSERY_LIMIT"]] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "adaptive_nursery_test",
            "runtime/tests/adaptive_nursery_test.c",
            true,
            &flags,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"adaptive nursery grows with retained graphs and shrinks after their release\n"
        );
    }
}

#[test]
fn dirty_card_groups_preserve_sparse_crossing_and_concurrent_writes() {
    let output = compile_and_run_with_flags(
        &workspace_root(),
        "dirty_card_groups_test",
        "runtime/tests/dirty_card_groups_test.c",
        true,
        &["-O2"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"dirty card groups preserve sparse, crossing and concurrent writes through collection\n"
    );
}

#[test]
fn zero_move_collections_skip_update_without_skipping_heap_work() {
    for verify in [false, true] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "no_move_update_test",
            "runtime/tests/no_move_update_test.c",
            verify,
            &["-O2"],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"zero-move full and minor preserve reclamation, promotion, pins and counters\n"
        );
    }
}

#[test]
fn allocation_statistics_include_detach_and_collection_baselines() {
    let output = compile_and_run(
        &workspace_root(),
        "allocation_statistics_test",
        "runtime/tests/allocation_statistics_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"allocation statistics survive collection, concurrent queries and repeated detach\n"
    );
}

#[test]
fn nursery_refill_retries_when_other_mutators_claim_the_capacity() {
    for optimization in ["-O0", "-O2"] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "nursery_refill_contention_test",
            "runtime/tests/nursery_refill_contention_test.c",
            true,
            &["-DSCOOP_THREAD_TESTING", optimization],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"nursery contention retries without false arena exhaustion\n"
        );
    }
}

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
    for flags in [vec![], vec!["-O2", "-DSCOOP_TEST_SURVIVOR"]] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "nursery_mutators_test",
            "runtime/tests/nursery_mutators_test.c",
            true,
            &flags,
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
fn retired_nursery_blocks_reset_metadata_and_preserve_promoted_graphs() {
    let output = compile_and_run(
        &workspace_root(),
        "nursery_reuse_test",
        "runtime/tests/nursery_reuse_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"retired nursery blocks reset metadata and preserve promoted graphs\n"
    );
}

#[test]
fn nursery_block_indexes_follow_promotion_reuse_and_full_reclamation() {
    let output = compile_and_run(
        &workspace_root(),
        "nursery_block_index_test",
        "runtime/tests/nursery_block_index_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"young and available-region indexes survive promotion, pin, reuse and full reclamation\n"
    );
}

#[test]
fn pointer_free_large_cards_do_not_hide_required_remembered_references() {
    let output = compile_and_run(
        &workspace_root(),
        "remembered_pointer_free_test",
        "runtime/tests/remembered_pointer_free_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"pointer-free large cards are skipped while reference cards retain and update young objects\n"
    );
}

#[test]
fn failed_promotion_reservation_falls_back_without_changing_the_graph() {
    for flags in [vec![], vec!["-O2", "-DSCOOP_TEST_SURVIVOR"]] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "nursery_fallback_test",
            "runtime/tests/nursery_fallback_test.c",
            true,
            &flags,
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
}
