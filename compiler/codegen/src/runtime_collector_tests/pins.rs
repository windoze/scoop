use super::{compile_and_run_with_flags, workspace_root};

#[test]
fn counted_pins_keep_objects_until_the_last_unpin() {
    check_pin_configurations(
        "counted_pin_test",
        b"counted pins preserve nested, swapped, large and concurrent roots\n",
    );
}

#[test]
fn scoped_pins_end_with_the_last_frame_and_compose_with_explicit_pins() {
    check_pin_configurations(
        "scoped_pin_test",
        b"scoped pins preserve roots and compose with explicit pins\n",
    );
}

fn check_pin_configurations(name: &str, expected: &[u8]) {
    let configurations: &[&[&str]] = &[
        &["-O0"],
        &["-O2"],
        &["-O0", "-DTEST_PIN_MINOR"],
        &["-O2", "-DTEST_PIN_MINOR"],
    ];
    for flags in configurations {
        let output = compile_and_run_with_flags(
            &workspace_root(),
            name,
            &format!("runtime/tests/{name}.c"),
            true,
            flags,
        );
        assert!(
            output.status.success(),
            "{flags:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected);
    }
}
