use super::compile::compile_and_run_configuration;
use super::{compile_and_run, compile_and_run_with_flags, workspace_root};

#[test]
fn retained_live_entries_are_current_before_reference_update() {
    let output = compile_and_run(
        &workspace_root(),
        "live_set_relocation_test",
        "runtime/tests/live_set_relocation_test.c",
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"live entries hold copied small, large and pointer-free objects before update; pins stay\n"
    );
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
