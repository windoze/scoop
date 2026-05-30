// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

use core::ffi::c_void;
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

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_alloc(size: u64) -> *mut c_void;

    fn scoop_enter_native(root_slots: *mut *mut *mut c_void, root_slots_len: u32);
    fn scoop_leave_native();

    fn scoop_gc_collect();
    fn scoop_gc_debug_heap_object_count() -> u64;
}

#[test]
#[cfg_attr(
    any(feature = "gc-minimal", feature = "gc-hosted"),
    ignore = "当前 backend（gc-minimal/gc-hosted）不支持 stop-the-world / 多线程 roots 枚举（该测试仅适用于支持这些能力的 backend）"
)]
fn gc_stop_the_world_scans_roots_on_all_registered_threads() {
    assert!(
        std::hint::black_box(GC_CAPABILITIES.stw && GC_CAPABILITIES.multi_thread_roots_enum),
        "该测试要求 STW + 多线程 roots 枚举能力；当前 backend={GC_BACKEND:?}, caps={GC_CAPABILITIES:?}"
    );

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

    // worker 线程：注册、分配一个对象、把对象写入 native_roots，然后等待 main 通知退出。
    let worker = std::thread::spawn(move || unsafe {
        scoop_thread_register();

        let mut keep = scoop_alloc(header_size + 8);
        assert!(!keep.is_null());

        // roots slots：数组元素为 `void**`（指向可读写的引用槽位）。
        let root0: *mut *mut c_void = &mut keep;
        let mut roots: [*mut *mut c_void; 1] = [root0];
        scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);

        ready_tx.send(()).unwrap();

        // 维持 InNative，直到 main 触发 GC 并通知退出。
        while !stop_worker.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }

        // 离开 InNative：清空 roots，允许对象被回收。
        scoop_leave_native();
        scoop_thread_unregister();
    });

    // main 线程：也注册并持有一个 roots，然后触发 GC 并验证两个线程的对象都未被回收。
    unsafe {
        // The main test thread is blocked in host synchronization while the worker may allocate
        // enough to trigger the block-pool hard GC path.
        scoop_enter_native(std::ptr::null_mut(), 0);
        let ready = ready_rx.recv();
        scoop_leave_native();
        ready.unwrap();

        let mut keep_main = scoop_alloc(header_size + 8);
        assert!(!keep_main.is_null());

        let root0: *mut *mut c_void = &mut keep_main;
        let mut roots: [*mut *mut c_void; 1] = [root0];
        scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);

        // 触发 GC：若只扫描当前线程 roots，则 worker 的对象会被 sweep 回收，heap count 将变为 1。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 2);

        // 清空 main roots，并让 worker 退出。
        scoop_leave_native();
        stop.store(true, Ordering::SeqCst);

        scoop_enter_native(std::ptr::null_mut(), 0);
        let joined = worker.join();
        scoop_leave_native();
        joined.unwrap();

        // 两个线程都不再持有 roots 后，再 collect 应回收所有对象。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        scoop_thread_unregister();
    }
}
