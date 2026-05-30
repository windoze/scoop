mod common;

#[link(name = "scooprt_test_core", kind = "static")]
unsafe extern "C" {
    fn scoop_test_gc_stackmap_multiframe_keepalive() -> isize;
}

#[test]
#[cfg_attr(
    any(feature = "gc-minimal", feature = "gc-hosted"),
    ignore = "当前 backend（gc-minimal/gc-hosted）不支持 stop-the-world / Parked 线程 stack walking（该测试仅适用于支持这些能力的 backend）"
)]
fn gc_stackmap_multiframe_keepalive() {
    assert!(
        std::hint::black_box(
            common::gc_supports_stw() && common::gc_supports_multi_thread_roots_enum()
        ),
        "该测试要求 STW + 多线程 roots 枚举能力；当前 backend={}, caps={}",
        common::gc_backend_name(),
        common::gc_capabilities_debug()
    );

    // P7-T02 why: the native helper asserts exact heap object counts after manual GCs;
    // disable soft pacing so no automatic safepoint collection perturbs those counters.
    unsafe {
        std::env::set_var("SCOOP_GC_PACING", "off");
    }

    let rc = unsafe { scoop_test_gc_stackmap_multiframe_keepalive() };
    assert_eq!(rc, 1, "multiframe keepalive 返回值异常：{rc}");
}
