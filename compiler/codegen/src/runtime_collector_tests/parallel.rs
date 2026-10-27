use super::compile::compile_and_run_configuration;
use super::{compile_and_run, compile_and_run_with_flags, workspace_root};

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
fn delayed_workers_observe_the_participants_of_the_published_phase() {
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
        b"delayed inactive workers retain the participants of their published phase\n"
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
    let output = compile_and_run(
        &workspace_root(),
        "parallel_mark_test",
        "runtime/tests/parallel_mark_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"1/2/4/8 markers preserve shared graphs, nested arrays, pins, minor roots and chains\n"
    );
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
