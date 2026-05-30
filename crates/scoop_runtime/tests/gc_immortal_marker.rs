mod common;

#[link(name = "scooprt_test_core", kind = "static")]
unsafe extern "C" {
    fn scoop_test_gc_immortal_marker_smoke() -> isize;
}

#[test]
fn immortal_marker_is_flag_gated_and_leaves_regular_marking_intact() {
    let rc = unsafe { scoop_test_gc_immortal_marker_smoke() };
    assert_eq!(
        rc,
        1,
        "immortal marker smoke 失败：backend={}, caps={}, rc={rc}",
        common::gc_backend_name(),
        common::gc_capabilities_debug()
    );
}
