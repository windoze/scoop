// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：回归 minor GC 中 old→nursery 边的保守晋升。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::mem;
    use core::ptr;

    type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
    type ScoopTypeTraceFn = Option<
        unsafe extern "C" fn(
            object: *mut c_void,
            visitor: ScoopGcTraceVisitor,
            ctx: *mut c_void,
        ) -> u64,
    >;
    type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

    #[repr(C)]
    struct ScoopGcObjectHeader {
        next: *mut ScoopGcObjectHeader,
        type_desc: *const ScoopTypeDescriptor,
        size_bytes: u64,
        flags: u32,
        mark: u32,
    }

    #[repr(C)]
    struct ScoopTypeDescriptor {
        abi_version: u32,
        flags: u32,
        size_bytes: u64,
        align_bytes: u64,
        trace_start_offset_bytes: u64,
        trace_bitmap_u64_len: u32,
        _reserved_u32: u32,
        trace_bitmap: *const u64,
        trace_fn: ScoopTypeTraceFn,
        release_fn: ScoopTypeReleaseFn,
        type_id: u64,
        parent_type_desc: *const ScoopTypeDescriptor,
        itable: *const c_void,
        vtable: *const c_void,
    }

    #[repr(C)]
    struct Container {
        header: ScoopGcObjectHeader,
        slot: *mut c_void,
    }

    #[repr(C)]
    struct LargeContainer {
        header: ScoopGcObjectHeader,
        slot: *mut c_void,
        pad: [u8; 40 * 1024],
    }

    #[repr(C)]
    struct Node {
        header: ScoopGcObjectHeader,
        child: *mut c_void,
        tag: u64,
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
        fn scoop_gc_collect_minor();
        fn scoop_gc_debug_heap_object_count() -> u64;
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
    fn immix_minor_preserves_old_to_nursery_edges() {
        unsafe {
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "4");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        let header_size = mem::size_of::<ScoopGcObjectHeader>() as u64;
        let container_size = mem::size_of::<Container>() as u64;
        let large_container_size = mem::size_of::<LargeContainer>() as u64;
        let leaf_size = mem::size_of::<Leaf>() as u64;
        let node_size = mem::size_of::<Node>() as u64;
        let filler_size = header_size + 8 * 1024;

        let node_trace_bitmap: [u64; 1] = [0b1];
        let node_type_desc = ScoopTypeDescriptor {
            abi_version: 0,
            flags: 0,
            size_bytes: node_size,
            align_bytes: mem::align_of::<Node>() as u64,
            trace_start_offset_bytes: mem::offset_of!(Node, child) as u64,
            trace_bitmap_u64_len: node_trace_bitmap.len() as u32,
            _reserved_u32: 0,
            trace_bitmap: node_trace_bitmap.as_ptr(),
            trace_fn: None,
            release_fn: None,
            type_id: 0,
            parent_type_desc: ptr::null(),
            itable: ptr::null(),
            vtable: ptr::null(),
        };

        let mut container = unsafe { scoop_alloc(container_size) };
        assert!(!container.is_null());
        assert_eq!(immix_generation(container), GEN_NURSERY);

        let mut large = ptr::null_mut();
        let root0: *mut *mut c_void = &mut container;
        let root1: *mut *mut c_void = &mut large;
        let mut roots: [*mut *mut c_void; 2] = [root0, root1];
        unsafe {
            let container_ptr = container.cast::<Container>();
            (*container_ptr).slot = ptr::null_mut();
            scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);
        }

        // 先让 rooted container 经自动 minor 变成 old，作为后续 old→nursery store 的宿主。
        for _ in 0..4096 {
            let p = unsafe { scoop_alloc(filler_size) };
            assert!(!p.is_null());
            if immix_generation(container) == GEN_OLD {
                break;
            }
        }
        assert_eq!(immix_generation(container), GEN_OLD);

        // Large/fallback malloc object 也是 old-space；写入 nursery ref 时必须安全晋升且不能崩溃。
        large = unsafe { scoop_alloc(large_container_size) };
        assert!(!large.is_null());
        let large_young = unsafe { scoop_alloc(leaf_size) };
        assert!(!large_young.is_null());
        assert_eq!(immix_generation(large_young), GEN_NURSERY);
        unsafe {
            let large_ptr = large.cast::<LargeContainer>();
            (*large_ptr).slot = ptr::null_mut();
            let slot_addr = ptr::addr_of_mut!((*large_ptr).slot) as *mut c_void;
            assert_eq!(scoop_gc_write_barrier(slot_addr, large_young), large_young);
            assert_eq!((*large_ptr).slot, large_young);
        }
        assert_eq!(
            immix_generation(large_young),
            GEN_OLD,
            "large old object store must promote nursery value"
        );

        // 构造 parent 与 child 位于不同 nursery blocks；parent 被写入 old container 后，
        // parent block 的既有 child 引用也必须被闭包晋升，不能在下一次 minor 中被 reset。
        let child = unsafe { scoop_alloc(leaf_size) };
        assert!(!child.is_null());
        assert_eq!(immix_generation(child), GEN_NURSERY);
        let child_block = immix_block_base(child);
        unsafe {
            let leaf = child.cast::<Leaf>();
            (*leaf).header.type_desc = ptr::null();
            (*leaf).value = 0xCAFE_BABE_DEAD_BEEFu64;
        }

        let mut saw_second_nursery_block = false;
        for _ in 0..64 {
            let p = unsafe { scoop_alloc(filler_size) };
            assert!(!p.is_null());
            assert_eq!(immix_generation(p), GEN_NURSERY);
            if immix_block_base(p) != child_block {
                saw_second_nursery_block = true;
                break;
            }
        }
        assert!(saw_second_nursery_block, "expected a second nursery block");

        let parent = unsafe { scoop_alloc(node_size) };
        assert!(!parent.is_null());
        assert_eq!(immix_generation(parent), GEN_NURSERY);
        assert_ne!(immix_block_base(parent), child_block);
        unsafe {
            let node = parent.cast::<Node>();
            (*node).header.type_desc = &node_type_desc;
            (*node).child = child;
            (*node).tag = 0x0123_4567_89AB_CDEFu64;
        }

        unsafe {
            let container_ptr = container.cast::<Container>();
            let slot_addr = ptr::addr_of_mut!((*container_ptr).slot) as *mut c_void;
            assert_eq!(scoop_gc_write_barrier(slot_addr, parent), parent);
            assert_eq!((*container_ptr).slot, parent);
        }
        assert_eq!(immix_generation(parent), GEN_OLD);
        assert_eq!(
            immix_generation(child),
            GEN_OLD,
            "promoted nursery block must also promote its nursery reference closure"
        );

        unsafe {
            scoop_gc_collect_minor();
        }
        unsafe {
            let container_ptr = container.cast::<Container>();
            let parent_after = (*container_ptr).slot.cast::<Node>();
            assert_eq!((*parent_after).tag, 0x0123_4567_89AB_CDEFu64);
            assert_eq!((*parent_after).child, child);

            let child_after = (*parent_after).child.cast::<Leaf>();
            assert_eq!((*child_after).value, 0xCAFE_BABE_DEAD_BEEFu64);

            scoop_leave_native();
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);

            scoop_thread_unregister();
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS");
        }
    }
}
