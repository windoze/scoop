// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1406c（Immix mark-region + region sweep / holes 复用）。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::ptr;
    use std::collections::BTreeSet;

    #[repr(C)]
    struct ScoopGcObjectHeader {
        next: *mut ScoopGcObjectHeader,
        type_desc: *const c_void,
        size_bytes: u64,
        flags: u32,
        mark: u32,
    }

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_alloc(size: u64) -> *mut c_void;

        fn scoop_gc_collect();
        fn scoop_gc_debug_heap_object_count() -> u64;

        fn scoop_pin(obj: *mut c_void) -> u32;
        fn scoop_unpin(obj: *mut c_void) -> u32;
    }

    const IMMIX_BLOCK_SIZE: usize = 32 * 1024;

    fn immix_block_base(ptr: *mut c_void) -> usize {
        let p = ptr as usize;
        p & !(IMMIX_BLOCK_SIZE - 1)
    }

    #[test]
    fn immix_mark_region_reuses_holes_in_partial_blocks() {
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 尽量从干净状态开始（避免跨用例的全局状态影响）。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);

            let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
            // 选择“较大对象”：
            // - 能快速填满 block；
            // - pinned 住每个 block 的首个对象后，GC 会制造大量 holes；
            // - 若 holes 不能被复用，后续再分配会必然申请新 blocks。
            let object_size = header_size + 4096;

            let total_objects: usize = 200;
            let mut pinned_objects: Vec<*mut c_void> = Vec::new();
            let mut initial_blocks: BTreeSet<usize> = BTreeSet::new();

            for _ in 0..total_objects {
                let obj = scoop_alloc(object_size);
                assert!(!obj.is_null(), "scoop_alloc must return non-null");

                let base = immix_block_base(obj);
                if initial_blocks.insert(base) {
                    // 每个 block pin 住 1 个对象：确保 block 不会被整块 reset，
                    // 从而逼迫 allocator 必须复用 partial blocks 的 holes 才能避免分配新 block。
                    assert_eq!(scoop_pin(obj), 1, "pin must succeed for heap object");

                    // 写入哨兵到 payload（避开对象头），用于检测“holes 复用”是否发生越界覆盖。
                    let payload = (obj as *mut u8).add(header_size as usize);
                    ptr::write_bytes(payload, 0xCD, 64);

                    pinned_objects.push(obj);
                }
            }

            assert!(
                initial_blocks.len() >= 4,
                "test must span multiple blocks; blocks={}",
                initial_blocks.len()
            );

            // 1) 回收未 pin 的对象：此时 heap 上应仅剩下每个 block 的 pinned 对象。
            scoop_gc_collect();
            assert_eq!(
                scoop_gc_debug_heap_object_count(),
                pinned_objects.len() as u64,
                "after gc, only pinned objects must remain"
            );

            // pinned 对象的哨兵仍应存在。
            for &obj in &pinned_objects {
                let payload = (obj as *mut u8).add(header_size as usize);
                for i in 0..64usize {
                    let b = payload.add(i).read_volatile();
                    assert_eq!(b, 0xCD, "pinned object payload must not be overwritten");
                }
            }

            // 2) 再分配：必须复用 existing blocks 的 holes，而不是申请新 blocks。
            //
            // 为降低对“每个 block 具体能容纳多少对象”的依赖，这里只回填一部分对象。
            let second_wave = (total_objects.saturating_sub(pinned_objects.len())) / 2;
            assert!(second_wave > 0, "second wave must be non-zero");

            for _ in 0..second_wave {
                let obj = scoop_alloc(object_size);
                assert!(!obj.is_null(), "second wave allocations must succeed");

                let base = immix_block_base(obj);
                assert!(
                    initial_blocks.contains(&base),
                    "allocation must reuse existing immix blocks (base=0x{base:x})"
                );
            }

            // pinned 对象的哨兵仍应存在（防止 holes 复用覆盖 live 对象）。
            for &obj in &pinned_objects {
                let payload = (obj as *mut u8).add(header_size as usize);
                for i in 0..64usize {
                    let b = payload.add(i).read_volatile();
                    assert_eq!(b, 0xCD, "pinned object payload must not be overwritten");
                }
            }

            // 清理 pinned roots：解除 pin 后，GC 应能回收到 0。
            for &obj in &pinned_objects {
                assert_eq!(scoop_unpin(obj), 1, "unpin must succeed");
            }
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);

            scoop_thread_unregister();
        }
    }
}
