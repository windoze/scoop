// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 TODO T1412c（minor collect：nursery evacuation）。
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

    // 对齐 `runtime/c/scoop_gc.h` 的对象头与 type descriptor（test-only）。
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
    struct Leaf {
        header: ScoopGcObjectHeader,
        value: u64,
    }

    #[repr(C)]
    struct Node {
        header: ScoopGcObjectHeader,
        child: *mut c_void,
        tag: u64,
    }

    // 仅用于测试读取 block 头部字段：
    // - `magic`：用于确认对象位于 Immix block 内；
    // - `generation`：用于断言 minor 后对象从 nursery 搬迁到 old（或 pinned 晋升）。
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

        fn scoop_pin(obj: *mut c_void) -> u32;
        fn scoop_unpin(obj: *mut c_void) -> u32;

        fn scoop_gc_collect();
        fn scoop_gc_collect_minor();
        fn scoop_gc_debug_heap_object_count() -> u64;
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
    fn immix_minor_collect_evac_updates_roots_and_resets_nursery() {
        // 必须在第一次 runtime init 前设置（runtime 当前为进程级全局 init）。
        // Rust 2024：修改进程环境变量在并发场景下可能产生 UB，因此 `set_var/remove_var` 为 unsafe。
        // 该测试建议配合 `--test-threads=1` 串行执行。
        unsafe {
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "1");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 从干净状态开始：避免其它 runtime 分配影响断言。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        // 1) 构造：A(Node) -> B(Leaf)，并额外分配若干 nursery garbage。
        let leaf_size = mem::size_of::<Leaf>() as u64;
        let node_size = mem::size_of::<Node>() as u64;

        // Node 仅有一个 ref 字段（child），用 bitmap 描述：
        // - trace_start_offset_bytes 指向 `child` 字段；
        // - bitmap bit0=1 表示“从 start 开始的第 0 个 word 是引用槽位”。
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

        let b = unsafe { scoop_alloc(leaf_size) };
        assert!(!b.is_null());
        assert_eq!(immix_generation(b), GEN_NURSERY);

        unsafe {
            let leaf = b.cast::<Leaf>();
            (*leaf).header.type_desc = ptr::null();
            (*leaf).value = 0xABCD_EF01_2345_6789u64;
        }

        let a = unsafe { scoop_alloc(node_size) };
        assert!(!a.is_null());
        assert_eq!(immix_generation(a), GEN_NURSERY);

        unsafe {
            let node = a.cast::<Node>();
            (*node).header.type_desc = &node_type_desc;
            (*node).child = b;
            (*node).tag = 0x0123_4567_89AB_CDEFu64;
        }

        for _ in 0..10 {
            let p = unsafe { scoop_alloc(leaf_size) };
            assert!(!p.is_null());
            assert_eq!(immix_generation(p), GEN_NURSERY);
        }

        let before = unsafe { scoop_gc_debug_heap_object_count() };
        assert_eq!(
            before, 12,
            "expected 2 live + 10 garbage objects in nursery"
        );

        // 2) roots：Rust 测试代码本身不产生 stackmap roots；用 enter_native 注册 roots slots。
        let mut root_a = a;
        unsafe {
            let root0: *mut *mut c_void = &mut root_a;
            let mut roots: [*mut *mut c_void; 1] = [root0];
            scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);
        }

        // 3) minor collect：应将 live nursery 对象搬迁到 old，并从 heap.objects 中移除 garbage。
        let old_a = root_a as usize;
        unsafe {
            scoop_gc_collect_minor();
        }
        let new_a = root_a as usize;
        assert_ne!(
            old_a, new_a,
            "expected Node to be evacuated to a new address"
        );
        assert_eq!(
            immix_generation(root_a),
            GEN_OLD,
            "expected evacuated Node to live in old generation"
        );

        // 4) A/B 的 payload 与引用修复必须正确（A.child 应被更新为 B 的新地址）。
        unsafe {
            let node = root_a.cast::<Node>();
            assert_eq!((*node).tag, 0x0123_4567_89AB_CDEFu64);

            let child = (*node).child;
            assert!(!child.is_null());
            assert_eq!(
                immix_generation(child),
                GEN_OLD,
                "expected evacuated child to live in old generation"
            );

            let leaf = child.cast::<Leaf>();
            assert_eq!((*leaf).value, 0xABCD_EF01_2345_6789u64);
        }

        // 5) nursery garbage 应被回收（仅保留 A/B 两个 live 对象的 to-space 副本）。
        let after = unsafe { scoop_gc_debug_heap_object_count() };
        assert_eq!(
            after, 2,
            "expected nursery garbage to be reclaimed by minor"
        );

        // 6) nursery reset 后应允许继续分配 nursery 对象（而不是持续落到 old）。
        let fresh = unsafe { scoop_alloc(leaf_size) };
        assert!(!fresh.is_null());
        assert_eq!(
            immix_generation(fresh),
            GEN_NURSERY,
            "expected nursery allocation to remain usable after minor reset"
        );

        // 7) pinned nursery：minor 必须先晋升 block（不可移动），随后仍可继续 nursery 分配。
        let pinned = unsafe { scoop_alloc(leaf_size) };
        assert!(!pinned.is_null());
        assert_eq!(immix_generation(pinned), GEN_NURSERY);
        assert_eq!(unsafe { scoop_pin(pinned) }, 1, "pin must succeed");

        unsafe {
            scoop_gc_collect_minor();
        }
        assert_eq!(
            immix_generation(pinned),
            GEN_OLD,
            "expected pinned nursery block to be promoted to old"
        );

        let fresh2 = unsafe { scoop_alloc(leaf_size) };
        assert!(!fresh2.is_null());
        assert_eq!(
            immix_generation(fresh2),
            GEN_NURSERY,
            "expected nursery allocation to remain usable after pinned promotion"
        );

        unsafe {
            scoop_leave_native();

            // cleanup：unpin 后 major collect 应能回收全部对象（Rust 侧无 stackmap roots）。
            assert_eq!(scoop_unpin(pinned), 1);
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);

            scoop_thread_unregister();

            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS");
        }
    }
}
