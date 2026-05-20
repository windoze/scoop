mod common;

#[link(name = "scooprt_test_core", kind = "static")]
unsafe extern "C" {
    fn scoop_test_gc_stack_walking_unwind_smoke() -> isize;
}

#[test]
fn gc_stack_walking_can_enumerate_frames_from_captured_ctx() {
    if !common::gc_supports_stw() {
        // non-STW backends（gc-minimal/gc-hosted）没有 park 语义，该测试直接跳过。
        return;
    }

    let rc = unsafe { scoop_test_gc_stack_walking_unwind_smoke() };
    assert_eq!(
        rc,
        1,
        "stack walking unwind smoke 失败：backend={}, caps={}, rc={rc}",
        common::gc_backend_name(),
        common::gc_capabilities_debug()
    );
}
