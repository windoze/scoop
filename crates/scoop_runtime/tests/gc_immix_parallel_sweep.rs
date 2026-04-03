// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1409c2（并行 region sweep v0；可开关）。
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

    fn run_scenario(enable_parallel_sweep: bool) {
        eprintln!(
            "[gc_immix_parallel_sweep] run_scenario: parallel_sweep={}",
            if enable_parallel_sweep { 1 } else { 0 }
        );

        // 独立于并行标记：测试中强制关闭并行 mark，避免外部环境变量影响可重复性。
        let prev_mark_env = std::env::var("SCOOP_GC_IMMIX_PARALLEL_MARK").ok();
        unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_MARK") };

        let prev_sweep_env = std::env::var("SCOOP_GC_IMMIX_PARALLEL_SWEEP").ok();
        match enable_parallel_sweep {
            true => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP", "4") },
            false => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP") },
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 尽量从干净状态开始（避免跨用例的全局状态影响）。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        unsafe {
            let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
            // 选择“较大对象”，逼迫快速跨 block，并在 region sweep 后产生明显 holes。
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

                    // 写入哨兵到 payload（避开对象头），用于检测 holes 复用是否发生越界覆盖。
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

        match prev_sweep_env {
            Some(v) => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP", v) },
            None => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP") },
        }

        match prev_mark_env {
            Some(v) => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_MARK", v) },
            None => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_MARK") },
        }
    }

    #[test]
    fn immix_parallel_sweep_toggle_smoke() {
        // 先在“关闭”模式下跑一轮，再在“开启”模式下跑一轮：避免依赖外部环境变量。
        eprintln!("[gc_immix_parallel_sweep] scenario: parallel_sweep=0");
        run_scenario(false);

        eprintln!("[gc_immix_parallel_sweep] scenario: parallel_sweep=1");
        run_scenario(true);
    }
}
