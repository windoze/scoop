// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1412b（Immix nursery：bump-only + 上限）。
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

    // 仅用于测试读取 block 头部字段：
    // - `magic`：用于确认该对象确实位于 Immix block 内；
    // - `generation`：用于确认 nursery 满后会触发 minor 并恢复可分配状态。
    #[repr(C)]
    struct ScoopGcImmixBlockHeader {
        magic: u64,
        generation: u8,
    }

    const IMMIX_BLOCK_SIZE: usize = 32 * 1024;
    const IMMIX_BLOCK_MAGIC: u64 = 0x5343_4F4F_5049_4D4D; // "SCOOPIMM"

    const GEN_OLD: u8 = 0;
    const GEN_NURSERY: u8 = 1;

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_alloc(size: u64) -> *mut c_void;
        fn scoop_enter_native(root_slots: *mut *mut *mut c_void, root_slots_len: u32);
        fn scoop_leave_native();

        fn scoop_gc_collect();
        fn scoop_gc_debug_heap_object_count() -> u64;
        fn scoop_gc_debug_heap_bytes_freed() -> u64;
        fn scoop_gc_debug_heap_gc_cycles() -> u64;
    }

    fn immix_block_base(ptr: *const c_void) -> *const ScoopGcImmixBlockHeader {
        let addr = ptr as usize;
        let base = addr & !(IMMIX_BLOCK_SIZE - 1);
        base as *const ScoopGcImmixBlockHeader
    }

    #[test]
    fn immix_nursery_full_runs_minor_and_reuses_nursery() {
        // 必须在第一次 runtime init 前设置（runtime 当前为进程级全局 init）。
        // Rust 2024：修改进程环境变量在并发场景下可能产生 UB，因此 `set_var/remove_var` 为 unsafe。
        // 该测试使用 `--test-threads=1` 串行执行，且在 runtime init 前完成设置。
        unsafe {
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "4");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 尽量从干净状态开始：避免其它 runtime 分配影响断言（以及便于定位回归）。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let live_size = header_size + 64;
        let garbage_size = header_size + 512;

        let cycles_before = unsafe { scoop_gc_debug_heap_gc_cycles() };
        let freed_before = unsafe { scoop_gc_debug_heap_bytes_freed() };

        let mut live = unsafe { scoop_alloc(live_size) };
        assert!(!live.is_null());
        assert_eq!(
            unsafe { (*immix_block_base(live)).generation },
            GEN_NURSERY,
            "expected initial live object in nursery"
        );

        let root0: *mut *mut c_void = &mut live;
        let mut roots: [*mut *mut c_void; 1] = [root0];
        unsafe {
            scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);
        }

        // 混合 live/dead workload：`live` 通过 native root 保活，其余对象应由自动 minor 回收。
        for _ in 0..2000 {
            let p = unsafe { scoop_alloc(garbage_size) };
            assert!(!p.is_null());

            let block = immix_block_base(p);
            let magic = unsafe { (*block).magic };
            assert_eq!(
                magic, IMMIX_BLOCK_MAGIC,
                "expected immix block allocation (magic=0x{magic:016x})"
            );

            let generation = unsafe { (*block).generation };
            assert!(
                generation == GEN_NURSERY || generation == GEN_OLD,
                "unknown immix block generation: {generation}"
            );
        }

        let cycles_after = unsafe { scoop_gc_debug_heap_gc_cycles() };
        let freed_after = unsafe { scoop_gc_debug_heap_bytes_freed() };
        assert!(
            cycles_after > cycles_before,
            "expected nursery-full allocation to trigger minor GC"
        );
        assert!(
            freed_after > freed_before,
            "expected automatic minor GC to reclaim dead nursery bytes"
        );

        let fresh = unsafe { scoop_alloc(live_size) };
        assert!(!fresh.is_null());
        assert_eq!(
            unsafe { (*immix_block_base(fresh)).generation },
            GEN_NURSERY,
            "expected nursery allocation to remain usable after automatic minor GC"
        );

        unsafe {
            scoop_leave_native();

            // major collect 应能回收所有无 roots 的对象。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);

            scoop_thread_unregister();
        }

        // 清理 env：避免影响该进程内未来可能新增的其它测试。
        unsafe {
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS");
        }
    }
}
