use super::compile::compile_and_run_configuration;
use super::{compile_and_run_with_flags, workspace_root};

#[test]
fn marking_preserves_exact_live_lines_across_bitmap_words_and_block_edges() {
    for (stress, optimization) in [(false, "-O2"), (true, "-O0")] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            "mark_lines_test",
            "runtime/tests/mark_lines_test.c",
            stress,
            stress,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "stress={stress}, optimization={optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"marking preserves exact live lines across bitmap words and block edges\n"
        );
    }
}

#[test]
fn live_chunk_reuse_preserves_graphs_and_releases_unused_capacity() {
    for (stress, optimization) in [(false, "-O2"), (true, "-O0")] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            "mark_batch_test",
            "runtime/tests/mark_batch_test.c",
            stress,
            stress,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "stress={stress}, optimization={optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"live chunk reuse preserves exact scans, relocation, shrinking and shutdown\n"
        );
    }
}

#[test]
fn remembered_slots_preserve_relocation_ownership_and_incomplete_record_fallback() {
    for (stress, optimization) in [(false, "-O2"), (true, "-O0")] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            "remembered_slots_test",
            "runtime/tests/remembered_slots_test.c",
            stress,
            stress,
            &[optimization, "-Dmalloc=scoop_test_remembered_malloc"],
        );
        assert!(
            output.status.success(),
            "stress={stress}, optimization={optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"remembered slots preserve relocation, group ownership and incomplete-record fallback\n"
        );
    }
}

#[test]
fn bounded_marking_counts_actual_array_and_remembered_references() {
    for (stress, optimization) in [(false, "-O2"), (true, "-O0")] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            "mark_slice_references_test",
            "runtime/tests/mark_slice_references_test.c",
            stress,
            stress,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "stress={stress}, optimization={optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"bounded marking counts actual array and remembered references before draining\n"
        );
    }
}

#[test]
fn wrapped_and_growing_queues_preserve_owner_and_thief_order() {
    for optimization in ["-O0", "-O2"] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "mark_queue_order_test",
            "runtime/tests/mark_queue_order_test.c",
            false,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"wrapped and growing queues preserve owner and thief order for every worker\n"
        );
    }
}

#[test]
fn parallel_slices_share_work_limits_and_finishing_drains_retained_tasks() {
    let output = compile_and_run_with_flags(
        &workspace_root(),
        "mark_slice_quanta_test",
        "runtime/tests/mark_slice_quanta_test.c",
        false,
        &["-O2"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"parallel slices share their work limit and finishing drains all retained tasks\n"
    );
}

#[test]
fn new_work_notifies_one_marker_and_completion_releases_all_waiters() {
    let output = compile_and_run_with_flags(
        &workspace_root(),
        "mark_work_notification_test",
        "runtime/tests/mark_work_notification_test.c",
        false,
        &[
            "-O2",
            "-Dpthread_cond_signal=scoop_test_mark_signal",
            "-Dpthread_cond_broadcast=scoop_test_mark_broadcast",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"new work wakes one waiting marker; completion releases every phase participant\n"
    );
}

#[test]
fn mark_region_views_preserve_collisions_movement_and_mapping_reuse() {
    for (stress, optimization) in [(false, "-O0"), (false, "-O2"), (true, "-O2")] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            "mark_region_cache_test",
            "runtime/tests/mark_region_cache_test.c",
            stress,
            stress,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "stress={stress}, optimization={optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"colliding region views preserve moved graphs across mapping retirement and reuse\n"
        );
    }
}

#[test]
fn short_work_preserves_the_published_phase_and_bounds_fixed_participants() {
    let output = compile_and_run_with_flags(
        &workspace_root(),
        "mark_phase_participation_test",
        "runtime/tests/mark_phase_participation_test.c",
        false,
        &["-O2", "-Dpthread_cond_wait=scoop_test_phase_wait"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"short work preserves the published phase; fixed work uses only needed participants\n"
    );
}

#[test]
fn parallel_full_reservations_own_hole_blocks_and_recover_from_failure() {
    let output = compile_and_run_with_flags(
        &workspace_root(),
        "full_parallel_reservation_test",
        "runtime/tests/full_parallel_reservation_test.c",
        true,
        &["-Dcalloc=scoop_test_full_copy_calloc"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"parallel full reservations own complete hole blocks and recover from partial failure\n"
    );
}

#[test]
fn copy_group_failure_preserves_the_graph_and_subsequent_copying() {
    let output = compile_and_run_with_flags(
        &workspace_root(),
        "copy_group_failure_test",
        "runtime/tests/copy_group_failure_test.c",
        true,
        &["-O2", "-Dcalloc=scoop_test_copy_calloc"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"copy grouping failure preserves the source graph and subsequent copying\n"
    );
}

#[test]
fn retained_live_entries_are_current_before_reference_update() {
    for optimization in ["-O0", "-O2"] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "live_set_relocation_test",
            "runtime/tests/live_set_relocation_test.c",
            true,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"live entries hold copied small, large and pointer-free objects before update; pins stay\n"
        );
    }
}

#[test]
fn serial_and_parallel_markers_preserve_shared_graphs_and_repeated_collection() {
    for (stress, optimization) in [(false, "-O2"), (true, "-O0")] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            "parallel_mark_test",
            "runtime/tests/parallel_mark_test.c",
            stress,
            stress,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "stress={stress}, optimization={optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"1/2/4/8/16 markers preserve shared graphs, nested arrays, pins, minor roots and chains\n"
        );
    }
}

#[test]
fn parallel_update_preserves_native_root_aliases_and_remembered_slots() {
    for (verify, stress, optimization) in [
        (false, false, "-O0"),
        (false, false, "-O2"),
        (true, false, "-O0"),
        (false, true, "-O0"),
    ] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            "parallel_update_test",
            "runtime/tests/parallel_update_test.c",
            verify,
            stress,
            &[optimization],
        );
        assert!(
            output.status.success(),
            "verify={verify}, stress={stress}, optimization={optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"parallel update preserves aliased native roots, pinned fields and remembered slots\n"
        );
    }
}

#[test]
fn worker_creation_failure_joins_started_threads_and_uses_the_same_serial_marker() {
    let output = compile_and_run_with_flags(
        &workspace_root(),
        "parallel_mark_failure_test",
        "runtime/tests/parallel_mark_failure_test.c",
        true,
        &[
            "-Dpthread_create=scoop_test_gc_pthread_create",
            "-Dpthread_join=scoop_test_gc_pthread_join",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"worker creation failure joins partial startup and preserves serial marking\n"
    );
}
