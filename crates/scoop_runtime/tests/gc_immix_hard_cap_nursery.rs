// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::mem;

    #[repr(C)]
    struct ScoopGcObjectHeader {
        next: *mut ScoopGcObjectHeader,
        type_desc: *const c_void,
        size_bytes: u64,
        flags: u32,
        mark: u32,
    }

    #[repr(C)]
    struct Leaf {
        header: ScoopGcObjectHeader,
        value: u64,
    }

    #[repr(C)]
    struct ScoopGcImmixBlockHeader {
        magic: u64,
        generation: u8,
    }

    const IMMIX_BLOCK_SIZE: usize = 32 * 1024;
    const IMMIX_BLOCK_MAGIC: u64 = 0x5343_4F4F_5049_4D4D; // "SCOOPIMM"
    const GEN_NURSERY: u8 = 1;

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_alloc(size: u64) -> *mut c_void;
        fn scoop_enter_native(root_slots: *mut *mut *mut c_void, root_slots_len: u32);
        fn scoop_leave_native();
        fn scoop_gc_collect();
        fn scoop_gc_collect_minor();

        fn scoop_gc_debug_heap_bytes_reserved() -> u64;
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
    fn immix_hard_cap_prevents_minor_tospace_growth() {
        unsafe {
            std::env::set_var("SCOOP_GC_PACING", "off");
            std::env::set_var("SCOOP_GC_MAX_HEAP_BYTES", IMMIX_BLOCK_SIZE.to_string());
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "1");
            std::env::remove_var("SCOOP_GC_STRESS");
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BYTES");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
            scoop_gc_collect();
        }

        let leaf_size = mem::size_of::<Leaf>() as u64;
        let mut live = unsafe { scoop_alloc(leaf_size) };
        assert!(!live.is_null(), "initial nursery allocation must fit");
        assert_eq!(
            immix_generation(live),
            GEN_NURSERY,
            "test setup requires a live nursery object"
        );
        assert!(
            unsafe { scoop_gc_debug_heap_bytes_reserved() } <= IMMIX_BLOCK_SIZE as u64,
            "initial nursery block must stay within the cap"
        );

        unsafe {
            let root0: *mut *mut c_void = &mut live;
            let mut roots: [*mut *mut c_void; 1] = [root0];
            scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);
            scoop_gc_collect_minor();
        }

        assert!(
            unsafe { scoop_gc_debug_heap_bytes_reserved() } <= IMMIX_BLOCK_SIZE as u64,
            "minor evacuation must not allocate to-space past SCOOP_GC_MAX_HEAP_BYTES"
        );
        assert_eq!(
            immix_generation(live),
            GEN_NURSERY,
            "minor collection should abort instead of growing to-space past the hard cap"
        );

        unsafe {
            scoop_leave_native();
            scoop_gc_collect();
            scoop_thread_unregister();
        }
    }
}
