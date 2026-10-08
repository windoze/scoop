use super::{compile_and_run_with_flags, workspace_root};

#[test]
fn native_safe_returns_and_silent_publication_preserve_moving_roots() {
    for flags in [
        vec!["-DSCOOP_THREAD_TESTING", "-O0"],
        vec!["-DSCOOP_THREAD_TESTING", "-O2"],
        vec!["-DSCOOP_THREAD_TESTING", "-DTEST_MINOR_GC", "-O0"],
        vec!["-DSCOOP_THREAD_TESTING", "-DTEST_MINOR_GC", "-O2"],
    ] {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "native_safe_handshake_test",
            "runtime/tests/native_safe_handshake_test.c",
            true,
            &flags,
        );
        assert!(
            output.status.success(),
            "{flags:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"native-safe return, publication and moving-root interleavings passed\n"
        );
    }
}
