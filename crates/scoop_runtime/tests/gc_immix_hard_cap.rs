// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

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
        fn scoop_pin(obj: *mut c_void) -> u32;
        fn scoop_unpin(obj: *mut c_void) -> u32;

        fn scoop_gc_debug_heap_bytes_reserved() -> u64;
        fn scoop_gc_debug_heap_gc_cycles() -> u64;
    }

    #[test]
    fn immix_hard_cap_reuses_after_gc_and_returns_null_when_live() {
        unsafe {
            // P7-T02 why: this test asserts exact reserved-byte and GC-cycle counters for the
            // hard-cap retry path, so soft pacing must not add unrelated collections.
            std::env::set_var("SCOOP_GC_PACING", "off");
            std::env::set_var("SCOOP_GC_MAX_HEAP_BYTES", IMMIX_BLOCK_SIZE.to_string());
            std::env::remove_var("SCOOP_GC_STRESS");
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BYTES");
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
            scoop_gc_collect();
        }

        let object_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64 + 24 * 1024;

        let first = unsafe { scoop_alloc(object_size) };
        assert!(
            !first.is_null(),
            "first block-sized allocation must fit under the cap"
        );
        assert!(
            unsafe { scoop_gc_debug_heap_bytes_reserved() } <= IMMIX_BLOCK_SIZE,
            "initial allocation must not reserve past the hard cap"
        );

        let cycles_before_reuse = unsafe { scoop_gc_debug_heap_gc_cycles() };
        let reused = unsafe { scoop_alloc(object_size) };
        assert!(
            !reused.is_null(),
            "unrooted garbage should be reclaimed before hard-cap growth is rejected"
        );
        assert!(
            unsafe { scoop_gc_debug_heap_gc_cycles() } > cycles_before_reuse,
            "near-cap allocation must run a full GC before reusing a block"
        );
        assert!(
            unsafe { scoop_gc_debug_heap_bytes_reserved() } <= IMMIX_BLOCK_SIZE,
            "reuse after GC must stay within the hard cap"
        );

        assert_eq!(
            unsafe { scoop_pin(reused) },
            1,
            "pin must keep the block live"
        );
        let cycles_before_oom = unsafe { scoop_gc_debug_heap_gc_cycles() };
        let over_cap = unsafe { scoop_alloc(object_size) };
        assert!(
            over_cap.is_null(),
            "live heap at the hard cap must make further block growth return NULL"
        );
        assert!(
            unsafe { scoop_gc_debug_heap_gc_cycles() } > cycles_before_oom,
            "hard-cap OOM must happen only after the full-GC retry"
        );
        assert!(
            unsafe { scoop_gc_debug_heap_bytes_reserved() } <= IMMIX_BLOCK_SIZE,
            "failed allocation must not grow reserved heap bytes"
        );

        let cycles_before_large_oom = unsafe { scoop_gc_debug_heap_gc_cycles() };
        let large = unsafe { scoop_alloc(IMMIX_BLOCK_SIZE + object_size) };
        assert!(
            large.is_null(),
            "large-object malloc fallback must also respect the hard cap"
        );
        assert!(
            unsafe { scoop_gc_debug_heap_gc_cycles() } > cycles_before_large_oom,
            "large-object hard-cap OOM must also retry after full GC"
        );

        unsafe {
            assert_eq!(scoop_unpin(reused), 1, "unpin must succeed");
            scoop_gc_collect();
            scoop_thread_unregister();
        }
    }
}
