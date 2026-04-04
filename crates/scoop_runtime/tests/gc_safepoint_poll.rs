// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_gc_collect();
    fn scoop_gc_safepoint_poll();
}

#[test]
#[cfg_attr(
    any(feature = "gc-minimal", feature = "gc-hosted"),
    ignore = "当前 backend（gc-minimal/gc-hosted）不支持 stop-the-world（该测试仅适用于支持这些能力的 backend）"
)]
fn gc_safepoint_poll_can_park_and_resume_other_threads() {
    assert!(
        GC_CAPABILITIES.stw,
        "该测试要求 STW 能力；当前 backend={GC_BACKEND:?}, caps={GC_CAPABILITIES:?}"
    );

    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
    }

    let stop = Arc::new(AtomicBool::new(false));
    let poll_count = Arc::new(AtomicU64::new(0));

    let stop_worker = stop.clone();
    let poll_count_worker = poll_count.clone();

    // worker：循环 poll，便于 STW 在请求时 park；并用计数器观测是否能在 GC 前后持续推进。
    let worker = std::thread::spawn(move || unsafe {
        scoop_thread_register();

        while !stop_worker.load(Ordering::SeqCst) {
            scoop_gc_safepoint_poll();
            poll_count_worker.fetch_add(1, Ordering::SeqCst);
            std::thread::yield_now();
        }

        scoop_thread_unregister();
    });

    // 等待 worker 至少运行一会儿，避免在其尚未进入 poll 循环前触发 GC 导致结果不稳定。
    let t0 = Instant::now();
    while poll_count.load(Ordering::SeqCst) < 128 {
        if t0.elapsed() > Duration::from_secs(2) {
            panic!("worker 未能进入 poll 循环（可能未被调度或发生死锁）");
        }
        std::thread::yield_now();
    }

    // 触发一次 GC：若 poll/协议有误，collect 可能卡死在等待线程 park 上。
    unsafe {
        scoop_gc_collect();
    }

    // GC 完成后，worker 仍应继续推进（即从 park 中恢复到 Running）。
    let after = poll_count.load(Ordering::SeqCst);
    let t1 = Instant::now();
    while poll_count.load(Ordering::SeqCst) == after {
        if t1.elapsed() > Duration::from_secs(2) {
            panic!("GC 后 worker 未恢复执行（可能卡在 park 或未被唤醒）");
        }
        std::thread::yield_now();
    }

    stop.store(true, Ordering::SeqCst);
    worker.join().unwrap();

    unsafe {
        scoop_thread_unregister();
    }
}
