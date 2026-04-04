// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

unsafe extern "C" {
    fn scoop_test_gc_stackmap_roots_enum_smoke() -> isize;
}

#[test]
#[cfg_attr(
    any(feature = "gc-minimal", feature = "gc-hosted"),
    ignore = "当前 backend（gc-minimal/gc-hosted）不支持 stop-the-world / Parked 线程 stack walking（该测试仅适用于支持这些能力的 backend）"
)]
fn gc_stackmap_roots_enum_smoke() {
    assert!(
        GC_CAPABILITIES.stw && GC_CAPABILITIES.multi_thread_roots_enum,
        "该测试要求 STW + 多线程 roots 枚举能力；当前 backend={GC_BACKEND:?}, caps={GC_CAPABILITIES:?}"
    );

    let rc = unsafe { scoop_test_gc_stackmap_roots_enum_smoke() };
    assert_eq!(rc, 1, "smoke 返回值异常：{rc}");
}
