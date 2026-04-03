// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-hosted` backend 下启用：
// - hosted backend 不支持 stop-the-world / 多线程 roots 枚举；
// - 因此当检测到多个线程已注册时，GC collect 必须退化为 no-op（宁可泄漏也不错误回收）。
#[cfg(feature = "gc-hosted")]
mod hosted {
    use core::ffi::c_void;
    use core::ptr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;

    #[repr(C)]
    struct ScoopGcObjectHeader {
        next: *mut ScoopGcObjectHeader,
        type_desc: *const c_void,
        size_bytes: u64,
        flags: u32,
        mark: u32,
    }

    // 对齐 `runtime/c/scoop_gc.h` 的 `ScoopGcFrame`（root_count=1 的固定版本）。
    #[repr(C)]
    struct ScoopGcFrame {
        prev: *mut ScoopGcFrame,
        root_count: u32,
        _reserved_u32: u32,
        roots: [*mut c_void; 1],
    }

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_alloc(size: u64) -> *mut c_void;

        fn scoop_gc_frame_push(frame: *mut ScoopGcFrame);
        fn scoop_gc_frame_pop(frame: *mut ScoopGcFrame);

        fn scoop_gc_collect();
        fn scoop_gc_debug_heap_object_count() -> u64;
    }

    #[test]
    fn hosted_collect_is_noop_when_multiple_threads_are_registered() {
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            // 确保起始为干净状态（即便未来 init 引入分配，这里也能自洽）。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel::<()>();

        // worker：注册 + 分配 + 写入 roots，然后等待 main 触发 “多线程期间的 collect no-op”。
        let worker = std::thread::spawn(move || unsafe {
            scoop_thread_register();

            let keep = scoop_alloc(header_size + 8);
            assert!(!keep.is_null());

            let mut frame = ScoopGcFrame {
                prev: ptr::null_mut(),
                root_count: 1,
                _reserved_u32: 0,
                roots: [keep],
            };
            scoop_gc_frame_push(&mut frame);

            ready_tx.send(()).unwrap();

            while !stop_worker.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }

            scoop_gc_frame_pop(&mut frame);
            scoop_thread_unregister();
        });

        unsafe {
            ready_rx.recv().unwrap();

            // main：也分配并持有 1 个 roots，然后尝试 collect。
            let keep_main = scoop_alloc(header_size + 8);
            assert!(!keep_main.is_null());

            let mut frame = ScoopGcFrame {
                prev: ptr::null_mut(),
                root_count: 1,
                _reserved_u32: 0,
                roots: [keep_main],
            };
            scoop_gc_frame_push(&mut frame);

            // 由于当前存在两个已注册线程，hosted backend 必须退化为 no-op：
            // - 不应崩溃；
            // - 不应错误回收其它线程仍持有 roots 的对象。
            scoop_gc_collect();
            assert_eq!(
                scoop_gc_debug_heap_object_count(),
                2,
                "hosted backend must not sweep heap objects when multiple threads are registered"
            );

            // 结束 worker，并在其退出后再进行一次 collect：此时只剩 main 线程已注册，应可正常回收。
            stop.store(true, Ordering::SeqCst);
            worker.join().unwrap();

            scoop_gc_frame_pop(&mut frame);
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);

            scoop_thread_unregister();
        }
    }
}
