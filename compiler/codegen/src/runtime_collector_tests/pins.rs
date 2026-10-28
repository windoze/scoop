use super::{compile_and_run_with_flags, workspace_root};

#[test]
fn pin_bitmaps_are_lazy_and_reused_without_weakening_pin_lifetimes() {
    for (verify_heap, enable_gc_stress, optimization) in
        [(true, true, "-O0"), (false, false, "-O2")]
    {
        let flags = [optimization, "-Dcalloc=scoop_test_pin_bitmap_calloc"];
        let output = super::compile::compile_and_run_configuration(
            &workspace_root(),
            "lazy_pin_bitmap_test",
            "runtime/tests/lazy_pin_bitmap_test.c",
            verify_heap,
            enable_gc_stress,
            &flags,
        );
        assert!(
            output.status.success(),
            "{flags:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            b"pin bitmaps are lazy, shared by block, reusable and fail before publication\n"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("out of memory allocating pin bitmap")
        );
    }
}

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
