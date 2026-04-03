// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

unsafe extern "C" {
    fn scoop_test_gc_stack_walking_unwind_smoke() -> isize;
}

#[test]
fn gc_stack_walking_can_enumerate_frames_from_captured_ctx() {
    if !GC_CAPABILITIES.stw {
        // non-STW backends（gc-minimal/gc-hosted）没有 park 语义，该测试直接跳过。
        return;
    }

    let rc = unsafe { scoop_test_gc_stack_walking_unwind_smoke() };
    assert_eq!(
        rc, 1,
        "stack walking unwind smoke 失败：backend={GC_BACKEND:?}, caps={GC_CAPABILITIES:?}, rc={rc}"
    );
}

