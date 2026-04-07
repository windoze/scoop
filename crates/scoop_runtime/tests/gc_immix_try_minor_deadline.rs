// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 TODO T1412e（try-minor / deadline）。
#[cfg(feature = "gc-immix")]
mod immix {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_gc_collect();
        fn scoop_gc_safepoint_poll();
        fn scoop_gc_try_collect_minor(deadline_ms: u32) -> u32;
    }

    fn wait_until(deadline: Instant, mut cond: impl FnMut() -> bool) {
        while !cond() {
            if Instant::now() >= deadline {
                panic!("condition not met before deadline");
            }
            thread::yield_now();
        }
    }

    #[test]
    fn immix_try_minor_deadline_returns_and_cancels_stw() {
        // 必须在第一次 runtime init 前设置（runtime 当前为进程级全局 init）。
        // Rust 2024：修改进程环境变量在并发场景下可能产生 UB，因此 `set_var/remove_var` 为 unsafe。
        unsafe {
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "1");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
        }

        // poller：持续 poll，使其在 STW 请求时会 park；用于验证“deadline 放弃时会被唤醒”。
        let poller_registered = Arc::new(AtomicBool::new(false));
        let poller_stop = Arc::new(AtomicBool::new(false));
        let poller_polls = Arc::new(AtomicU64::new(0));
        let poller = {
            let poller_registered = Arc::clone(&poller_registered);
            let poller_stop = Arc::clone(&poller_stop);
            let poller_polls = Arc::clone(&poller_polls);
            thread::spawn(move || unsafe {
                scoop_thread_register();
                poller_registered.store(true, Ordering::SeqCst);

                while !poller_stop.load(Ordering::SeqCst) {
                    scoop_gc_safepoint_poll();
                    poller_polls.fetch_add(1, Ordering::SeqCst);
                    thread::yield_now();
                }

                scoop_thread_unregister();
            })
        };

        // stuck：注册但故意不 poll，导致协作式 STW 无法达成，从而触发 try-minor 的 deadline 放弃路径。
        let stuck_registered = Arc::new(AtomicBool::new(false));
        let stuck_stop = Arc::new(AtomicBool::new(false));
        let stuck = {
            let stuck_registered = Arc::clone(&stuck_registered);
            let stuck_stop = Arc::clone(&stuck_stop);
            thread::spawn(move || unsafe {
                scoop_thread_register();
                stuck_registered.store(true, Ordering::SeqCst);

                while !stuck_stop.load(Ordering::SeqCst) {
                    // 故意不调用任何 runtime safepoint/alloc/enter_native：保持 Running，阻塞 STW。
                    thread::yield_now();
                }

                scoop_thread_unregister();
            })
        };

        // 等待两条线程都注册完成（否则 STW 可能不会等到它们，导致测试不稳定）。
        wait_until(Instant::now() + Duration::from_secs(2), || {
            poller_registered.load(Ordering::SeqCst) && stuck_registered.load(Ordering::SeqCst)
        });

        // 等待 poller 至少跑一小段，确保其确实在 poll。
        wait_until(Instant::now() + Duration::from_secs(2), || {
            poller_polls.load(Ordering::SeqCst) >= 64
        });

        // 触发 try-minor：由于 stuck 不 poll，STW 无法在 deadline 内达成，应当超时返回且不死锁。
        let deadline_ms: u32 = 50;
        let start = Instant::now();
        let did_minor = unsafe { scoop_gc_try_collect_minor(deadline_ms) };
        let elapsed = start.elapsed();

        assert_eq!(
            did_minor, 0,
            "expected try-minor to give up due to STW deadline"
        );
        assert!(
            elapsed >= Duration::from_millis(deadline_ms as u64),
            "expected try-minor to wait roughly until deadline (elapsed={elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "expected try-minor to return promptly after deadline (elapsed={elapsed:?})"
        );

        // deadline 放弃必须撤销 STW 请求并唤醒已 park 线程：poller 需要恢复继续 poll。
        let before = poller_polls.load(Ordering::SeqCst);
        wait_until(Instant::now() + Duration::from_secs(2), || {
            poller_polls.load(Ordering::SeqCst) > before
        });

        // 清理 stuck（否则后续 major collect 会被它一直阻塞）。
        stuck_stop.store(true, Ordering::SeqCst);
        stuck.join().expect("stuck thread join");

        // 放弃 try-minor 后，仍应能进行一次 major collect（验证 STW 状态机未被破坏）。
        unsafe {
            scoop_gc_collect();
        }

        poller_stop.store(true, Ordering::SeqCst);
        poller.join().expect("poller thread join");

        unsafe {
            scoop_thread_unregister();
        }
    }
}
