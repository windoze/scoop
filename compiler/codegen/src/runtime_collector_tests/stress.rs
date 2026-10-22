use super::*;

#[test]
fn stable_references_and_heap_object_starts_follow_runtime_configuration() {
    let optimized = ["-O2", "-DNDEBUG"];
    for (name, verify, stress, flags) in [
        ("stable_regular", false, false, optimized.as_slice()),
        ("stable_debug", false, false, &[]),
        ("stable_stress", false, true, optimized.as_slice()),
        ("stable_verify", true, false, optimized.as_slice()),
    ] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            name,
            "runtime/tests/stable_reference_test.c",
            verify,
            stress,
            flags,
        );
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"stable references and heap object starts passed\n"
        );
    }
}

#[test]
fn stress_environment_requires_explicit_runtime_support() {
    for enabled in [false, true] {
        let output = compile_and_run_configuration(
            &workspace_root(),
            if enabled {
                "stress_enabled"
            } else {
                "stress_disabled"
            },
            "runtime/tests/stress_configuration_test.c",
            false,
            enabled,
            &[],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"stress configuration passed\n");
    }
}
