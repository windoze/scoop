use super::*;

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
