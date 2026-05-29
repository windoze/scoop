// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 P2-T02（block pool 耗尽先 full GC 再增长）。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;

    #[repr(C)]
    struct ScoopGcObjectHeader {
        next: *mut ScoopGcObjectHeader,
        type_desc: *const c_void,
        size_bytes: u64,
        flags: u32,
        mark: u32,
    }

    const IMMIX_BLOCK_SIZE: u64 = 32 * 1024;

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_alloc(size: u64) -> *mut c_void;
        fn scoop_gc_collect();

        fn scoop_gc_debug_heap_bytes_reserved() -> u64;
        fn scoop_gc_debug_heap_gc_cycles() -> u64;
    }

    #[test]
    fn immix_block_pool_exhaustion_collects_before_growing() {
        // Isolate the P2 hard trigger: disable soft pacing/stress/nursery so any GC cycle observed
        // during allocation must come from old-space block-pool exhaustion.
        unsafe {
            std::env::set_var("SCOOP_GC_PACING", "off");
            std::env::remove_var("SCOOP_GC_STRESS");
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BYTES");
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 从空堆开始，避免跨初始化状态影响 reserved-byte 断言。
            scoop_gc_collect();
        }

        let object_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64 + 24 * 1024;

        let mut previous_reserved = unsafe { scoop_gc_debug_heap_bytes_reserved() };
        let mut previous_cycles = unsafe { scoop_gc_debug_heap_gc_cycles() };
        let mut saw_collect_without_growth = false;

        for i in 0..128usize {
            let obj = unsafe { scoop_alloc(object_size) };
            assert!(!obj.is_null(), "allocation must succeed (i={i})");

            let reserved = unsafe { scoop_gc_debug_heap_bytes_reserved() };
            let cycles = unsafe { scoop_gc_debug_heap_gc_cycles() };
            if previous_reserved >= IMMIX_BLOCK_SIZE
                && reserved == previous_reserved
                && cycles > previous_cycles
            {
                saw_collect_without_growth = true;
                break;
            }

            previous_reserved = reserved;
            previous_cycles = cycles;
        }

        assert!(
            saw_collect_without_growth,
            "exhausting old-space blocks should collect and reuse reclaimable blocks before growing"
        );

        unsafe {
            scoop_gc_collect();
            scoop_thread_unregister();
        }
    }
}
