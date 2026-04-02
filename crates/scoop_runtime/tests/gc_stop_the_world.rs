#![cfg(feature = "gc-baseline")]

// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size: u64,
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
    fn scoop_gc_safepoint();
    fn scoop_gc_debug_heap_object_count() -> u64;
}

#[test]
fn gc_stop_the_world_scans_roots_on_all_registered_threads() {
    unsafe {
        scoop_runtime_init();
    }

    let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;

    unsafe {
        scoop_thread_register();

        // 清理起始状态（避免未来 init 引入分配导致该测试不稳定）。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);
    }

    // 用 AtomicBool 做最小同步：worker 在 safepoint 循环中等待 main 触发 GC 后通知退出。
    let stop = Arc::new(AtomicBool::new(false));
    let stop_worker = stop.clone();

    // worker ready 信号：确保 main 在触发 GC 前，worker 已完成“注册 + 写入 roots + 进入 safepoint 循环”。
    let (ready_tx, ready_rx) = mpsc::channel::<()>();

    // worker 线程：注册、分配一个对象、把对象写入 roots，然后不断进入 safepoint（便于 STW 暂停）。
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

        // 等待 main 触发 GC（协作式 STW：需要 safepoint 才会被暂停）。
        while !stop_worker.load(Ordering::SeqCst) {
            scoop_gc_safepoint();
            std::thread::yield_now();
        }

        scoop_gc_frame_pop(&mut frame);
        scoop_thread_unregister();
    });

    // main 线程：也注册并持有一个 roots，然后触发 GC 并验证两个线程的对象都未被回收。
    unsafe {
        ready_rx.recv().unwrap();

        let keep_main = scoop_alloc(header_size + 8);
        assert!(!keep_main.is_null());

        let mut frame = ScoopGcFrame {
            prev: ptr::null_mut(),
            root_count: 1,
            _reserved_u32: 0,
            roots: [keep_main],
        };
        scoop_gc_frame_push(&mut frame);

        // 触发 GC：若只扫描当前线程 roots，则 worker 的对象会被 sweep 回收，heap count 将变为 1。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 2);

        scoop_gc_frame_pop(&mut frame);
        stop.store(true, Ordering::SeqCst);

        worker.join().unwrap();

        // 两个线程都不再持有 roots 后，再 collect 应回收所有对象。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        scoop_thread_unregister();
    }
}
