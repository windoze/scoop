use super::{compile_and_run, compile_and_run_with_flags, workspace_root};

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
