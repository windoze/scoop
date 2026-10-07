use super::{compile_and_run_with_flags, workspace_root};

#[test]
fn counted_pins_keep_objects_until_the_last_unpin() {
    let configurations: &[&[&str]] = &[
        &["-O0"],
        &["-O2"],
        &["-O0", "-DTEST_PIN_MINOR"],
        &["-O2", "-DTEST_PIN_MINOR"],
    ];
    for flags in configurations {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            "counted_pin_test",
            "runtime/tests/counted_pin_test.c",
            true,
            flags,
        );
        assert!(
            output.status.success(),
            "{flags:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"counted pins preserve nested, swapped, large and concurrent roots\n"
        );
    }
}
