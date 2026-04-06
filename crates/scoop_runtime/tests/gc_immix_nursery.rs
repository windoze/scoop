// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1412b（Immix nursery：bump-only + 上限）。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use std::collections::HashSet;

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
    // - `generation`：用于确认 nursery 上限生效（超限后回退到 old blocks）。
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

        fn scoop_gc_collect();
        fn scoop_gc_debug_heap_object_count() -> u64;
    }

    fn immix_block_base(ptr: *const c_void) -> *const ScoopGcImmixBlockHeader {
        let addr = ptr as usize;
        let base = addr & !(IMMIX_BLOCK_SIZE - 1);
        base as *const ScoopGcImmixBlockHeader
    }

    #[test]
    fn immix_nursery_blocks_are_capped_and_fallback_to_old() {
        // 必须在第一次 runtime init 前设置（runtime 当前为进程级全局 init）。
        // Rust 2024：修改进程环境变量在并发场景下可能产生 UB，因此 `set_var/remove_var` 为 unsafe。
        // 该测试使用 `--test-threads=1` 串行执行，且在 runtime init 前完成设置。
        unsafe {
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "1");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 尽量从干净状态开始：避免其它 runtime 分配影响断言（以及便于定位回归）。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let obj_size = header_size + 256;

        let mut nursery_blocks: HashSet<usize> = HashSet::new();
        let mut saw_nursery = false;
        let mut saw_old = false;

        // 分配足够多的对象，确保超过 1 个 block 的容量（从而必须回退到 old）。
        for _ in 0..500 {
            let p = unsafe { scoop_alloc(obj_size) };
            assert!(!p.is_null());

            let block = immix_block_base(p);
            let magic = unsafe { (*block).magic };
            assert_eq!(
                magic, IMMIX_BLOCK_MAGIC,
                "expected immix block allocation (magic=0x{magic:016x})"
            );

            let generation = unsafe { (*block).generation };
            match generation {
                GEN_NURSERY => {
                    saw_nursery = true;
                    nursery_blocks.insert(block as usize);
                }
                GEN_OLD => {
                    saw_old = true;
                }
                other => panic!("unknown immix block generation: {other}"),
            }
        }

        // 上限为 1 block：nursery block 集合不得超过 1；且必须出现 old fallback。
        assert!(saw_nursery, "expected at least one nursery allocation");
        assert!(saw_old, "expected fallback to old after nursery fills");
        assert!(
            nursery_blocks.len() <= 1,
            "nursery blocks must be capped (got {})",
            nursery_blocks.len()
        );

        unsafe {
            // major collect（当前仅有 major）应能回收所有无 roots 的对象。
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
