// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

use std::sync::mpsc;
use std::time::Duration;

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_alloc(size: u64) -> *mut std::ffi::c_void;

    fn scoop_enter_native(root_slots: *mut *mut *mut std::ffi::c_void, root_slots_len: u32);
    fn scoop_leave_native();

    fn scoop_gc_collect();
    fn scoop_gc_debug_heap_object_count() -> u64;
}

#[test]
#[cfg_attr(
    any(feature = "gc-minimal", feature = "gc-hosted"),
    ignore = "当前 backend（gc-minimal/gc-hosted）不支持 stop-the-world（该测试仅适用于支持这些能力的 backend）"
)]
fn gc_enter_native_treats_innative_thread_as_ready_and_preserves_roots() {
    if !GC_CAPABILITIES.stw {
        // non-STW backends 没有线程状态机/park 协议，该测试直接跳过。
        return;
    }

    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
    }

    let base = unsafe { scoop_gc_debug_heap_object_count() };

    let (ready_tx, ready_rx) = mpsc::channel::<u64>();
    let (leave_tx, leave_rx) = mpsc::channel::<()>();

    // worker：进入 InNative，并通过 native_roots 保护一个仅由“native 侧”持有的 GC 指针。
    let worker = std::thread::spawn(move || unsafe {
        scoop_thread_register();

        // 分配一个对象：它不会出现在 shadow stack（Rust 测试不维护 shadow stack），因此若没有
        // native_roots 保护，下一次 GC 会把它当作垃圾回收。
        let mut obj = scoop_alloc(64);
        assert!(
            !obj.is_null(),
            "scoop_alloc 返回 NULL（OOM 或运行时未初始化）"
        );

        let after_alloc = scoop_gc_debug_heap_object_count();

        // roots slots：数组元素为 `void**`（指向可读写的引用槽位）。
        let root0: *mut *mut std::ffi::c_void = &mut obj;
        let mut roots: [*mut *mut std::ffi::c_void; 1] = [root0];

        scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);

        ready_tx
            .send(after_alloc)
            .expect("ready_tx: send failed (receiver dropped)");

        // 持续保持 InNative，直到主线程要求离开。
        leave_rx
            .recv()
            .expect("leave_rx: recv failed (sender dropped)");

        scoop_leave_native();
        scoop_thread_unregister();
    });

    // 等待 worker 完成分配并进入 InNative。
    let after_alloc = ready_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_else(|_| panic!("worker 未能在超时内进入 InNative：backend={GC_BACKEND:?}"));

    assert_eq!(
        after_alloc,
        base + 1,
        "预期只新增 1 个对象：backend={GC_BACKEND:?}, base={base}, after_alloc={after_alloc}"
    );

    // 触发一次 GC：若 InNative 未被视为“已就绪”，collect 可能会卡死在等待 worker park 上；
    // 若 native_roots 未被扫描，对象会在本轮被回收。
    unsafe {
        scoop_gc_collect();
    }

    let after_gc = unsafe { scoop_gc_debug_heap_object_count() };
    assert_eq!(
        after_gc,
        base + 1,
        "对象应在 InNative 期间被 native_roots 保活：backend={GC_BACKEND:?}, base={base}, after_gc={after_gc}"
    );

    // 离开 InNative 并退出线程；随后再次 GC，应当回收该对象。
    leave_tx.send(()).unwrap();
    worker.join().unwrap();

    unsafe {
        scoop_gc_collect();
    }

    let after_leave_gc = unsafe { scoop_gc_debug_heap_object_count() };
    assert_eq!(
        after_leave_gc, base,
        "离开 InNative 且线程退出后，对象应可被回收：backend={GC_BACKEND:?}, base={base}, after_leave_gc={after_leave_gc}"
    );

    unsafe {
        scoop_thread_unregister();
    }
}
