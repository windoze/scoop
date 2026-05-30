// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 TODO T1412d（写屏障 hook v0：promote-on-store）。
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

    #[repr(C)]
    struct Container {
        header: ScoopGcObjectHeader,
        slot: *mut c_void,
    }

    // 仅用于测试读取 block 头部字段：
    // - `magic`：用于确认该对象确实位于 Immix block 内；
    // - `generation`：用于确认 promote-on-store 把 nursery block 晋升为 old。
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

        // `void* scoop_gc_write_barrier(void* slot_addr, void* value)`
        fn scoop_gc_write_barrier(slot_addr: *mut c_void, value: *mut c_void) -> *mut c_void;
    }

    fn immix_block_base(ptr: *const c_void) -> *const ScoopGcImmixBlockHeader {
        let addr = ptr as usize;
        let base = addr & !(IMMIX_BLOCK_SIZE - 1);
        base as *const ScoopGcImmixBlockHeader
    }

    fn immix_generation(ptr: *const c_void) -> u8 {
        let block = immix_block_base(ptr);
        let magic = unsafe { (*block).magic };
        assert_eq!(
            magic, IMMIX_BLOCK_MAGIC,
            "expected immix block allocation (magic=0x{magic:016x})"
        );
        unsafe { (*block).generation }
    }

    #[test]
    fn immix_write_barrier_promotes_nursery_block_on_old_store() {
        // 必须在第一次 runtime init 前设置（runtime 当前为进程级全局 init）。
        // Rust 2024：修改进程环境变量在并发场景下可能产生 UB，因此 `set_var/remove_var` 为 unsafe。
        // 该 integration test 进程内只有一个测试函数，因此不会并发执行。
        unsafe {
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "1");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 尽量从干净状态开始，避免其它 runtime 分配影响断言。
            scoop_gc_collect();
        }

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let small_obj_size = header_size + 64;
        let container_size = core::mem::size_of::<Container>() as u64;

        // 1) 先分配容器，并通过 native root 保活；自动 minor 会把它搬到 old。
        let mut container = unsafe { scoop_alloc(container_size) };
        assert!(!container.is_null());
        assert_eq!(
            immix_generation(container),
            GEN_NURSERY,
            "expected initial container allocated in nursery"
        );
        let root0: *mut *mut c_void = &mut container;
        let mut roots: [*mut *mut c_void; 1] = [root0];
        unsafe {
            let container_ptr = container as *mut Container;
            (*container_ptr).slot = core::ptr::null_mut();
            scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);
        }

        // 2) 填满 nursery（上限 1 block），自动 minor 应把 rooted container 搬到 old。
        let filler_size = header_size + 8 * 1024;
        for _ in 0..4096 {
            let p = unsafe { scoop_alloc(filler_size) };
            assert!(!p.is_null());
            if immix_generation(container) == GEN_OLD {
                break;
            }
        }
        assert_eq!(
            immix_generation(container),
            GEN_OLD,
            "expected automatic minor to evacuate rooted container to old"
        );

        // 3) 再分配一个 nursery 对象（young），并用 write barrier 写入 old container。
        let young = unsafe { scoop_alloc(small_obj_size) };
        assert!(!young.is_null());
        assert_eq!(
            immix_generation(young),
            GEN_NURSERY,
            "expected young object allocated in nursery"
        );

        let container_ptr = container as *mut Container;
        unsafe {
            let slot_addr = core::ptr::addr_of_mut!((*container_ptr).slot) as *mut c_void;
            let written = scoop_gc_write_barrier(slot_addr, young);

            assert_eq!(
                written, young,
                "v0 write barrier must return the written value"
            );
            assert_eq!(
                (*container_ptr).slot,
                young,
                "write barrier must write value into slot"
            );
        }

        // 4) promote-on-store：young 所在的 nursery block 必须被晋升为 old，
        //    从而避免在未来 minor GC 中出现 old→nursery 指针。
        assert_eq!(
            immix_generation(young),
            GEN_OLD,
            "expected write barrier to promote the nursery block to old"
        );

        // 5) 晋升后仍应允许继续分配新的 nursery blocks（计数与当前 block 指针必须自洽）。
        let fresh = unsafe { scoop_alloc(small_obj_size) };
        assert!(!fresh.is_null());
        assert_eq!(
            immix_generation(fresh),
            GEN_NURSERY,
            "expected nursery allocation to remain usable after promotion"
        );

        unsafe {
            scoop_leave_native();

            // 本测试不依赖 GC roots（Rust 测试代码未走 stackmap roots 协议），因此在收尾处 collect 即可。
            scoop_gc_collect();
            scoop_thread_unregister();

            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS");
        }
    }
}
