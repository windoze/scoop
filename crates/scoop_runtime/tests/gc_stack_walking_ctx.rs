mod common;

#[link(name = "scooprt_test_core", kind = "static")]
unsafe extern "C" {
    fn scoop_test_gc_stack_walking_ctx_smoke() -> isize;
}

#[test]
fn gc_stack_walking_ctx_is_captured_and_cleared_across_stw() {
    if !common::gc_supports_stw() {
        // non-STW backends（gc-minimal/gc-hosted）没有 park 语义，该测试直接跳过。
        return;
    }

    let rc = unsafe { scoop_test_gc_stack_walking_ctx_smoke() };
    assert_eq!(
        rc,
        1,
        "stack walking ctx smoke 失败：backend={}, caps={}, rc={rc}",
        common::gc_backend_name(),
        common::gc_capabilities_debug()
    );
}
