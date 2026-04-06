// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1406b（Immix block/line allocator v0）。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::ptr;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    // `scoop_runtime_init()` 与 GC 全局状态是进程级别的；同一个 test binary 内并发跑多个
    // Immix 集成测试会互相干扰甚至死锁（例如 STW 等待其它测试线程“park”）。
    //
    // 约定：Immix 集成测试在同一个进程内串行执行；不影响其它 crate / test binary 的并行度。
    static GC_IMMIX_TEST_LOCK: Mutex<()> = Mutex::new(());

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
        fn scoop_gc_debug_heap_bytes_allocated() -> u64;
        fn scoop_gc_debug_heap_bytes_freed() -> u64;
    }

    #[test]
    fn immix_allocator_many_allocations_multiple_gc_cycles_do_not_explode() {
        let _lock = GC_IMMIX_TEST_LOCK.lock().unwrap();
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 起始清理：确保测试对跨用例的全局状态有一定鲁棒性。
            scoop_gc_collect();

            let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
            // 选一个“足够大但仍明显小于 32KiB block”的对象尺寸：
            // - 这样可以逼迫分配路径跨越多个 blocks；
            // - 若 block 不能被回收/复用，多轮循环会显著推高内存压力。
            let object_size = header_size + 4096;

            let cycles: usize = 25;
            let per_cycle: usize = 5000;

            for cycle in 0..cycles {
                for _ in 0..per_cycle {
                    let p = scoop_alloc(object_size);
                    assert!(
                        !p.is_null(),
                        "scoop_alloc must return non-null (cycle={cycle})"
                    );

                    // 触碰一部分内存以增加“真实压力”，避免纯 bump 指针分配在某些系统上
                    // 因 lazy commit 而掩盖 block 泄漏问题。
                    let bytes = object_size as usize;
                    let header_bytes = core::mem::size_of::<ScoopGcObjectHeader>();
                    let payload_len = bytes.saturating_sub(header_bytes);
                    let payload = (p as *mut u8).add(header_bytes);
                    ptr::write_bytes(payload, 0xA5, payload_len.min(256));
                    if bytes > 0 {
                        let last = (p as *mut u8).add(bytes - 1);
                        last.write_volatile(0x5A);
                    }
                }

                scoop_gc_collect();
                assert_eq!(
                    scoop_gc_debug_heap_object_count(),
                    0,
                    "after gc, heap object count must return to 0 (cycle={cycle})"
                );
            }

            // 统计字段不应出现“爆炸式增长”的异常：若每轮 GC 都回收了全部对象，
            // 那么累计 allocated/freed 应当相等（差值为 0）。
            let allocated = scoop_gc_debug_heap_bytes_allocated();
            let freed = scoop_gc_debug_heap_bytes_freed();
            assert_eq!(
                allocated, freed,
                "allocated bytes must equal freed bytes when no objects remain"
            );

            scoop_thread_unregister();
        }
    }

    #[test]
    fn immix_allocator_multithread_alloc_and_collect_smoke() {
        let _lock = GC_IMMIX_TEST_LOCK.lock().unwrap();
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let object_size = header_size + 64;

        let threads: usize = 4;
        let start = Arc::new(Barrier::new(threads + 1));
        let stop = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();
        for _ in 0..threads {
            let start = start.clone();
            let stop = stop.clone();
            handles.push(std::thread::spawn(move || unsafe {
                scoop_thread_register();
                start.wait();

                for i in 0..20_000usize {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }

                    let p = scoop_alloc(object_size);
                    assert!(!p.is_null());

                    // 触碰一小段 payload：避免某些平台下的 lazy commit 掩盖问题。
                    let header_bytes = core::mem::size_of::<ScoopGcObjectHeader>();
                    let payload = (p as *mut u8).add(header_bytes);
                    ptr::write_bytes(payload, 0xCC, 16);

                    if (i % 1024) == 0 {
                        std::thread::yield_now();
                    }
                }

                scoop_thread_unregister();
            }));
        }

        start.wait();

        for _ in 0..10 {
            unsafe { scoop_gc_collect() };
            std::thread::yield_now();
        }

        stop.store(true, Ordering::Relaxed);

        for h in handles {
            h.join().unwrap();
        }

        unsafe {
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
            scoop_thread_unregister();
        }
    }
}
