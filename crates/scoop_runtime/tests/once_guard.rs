// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

unsafe extern "C" {
    fn scoop_once_begin(guard_word: *mut u64) -> u32;
    fn scoop_once_end(guard_word: *mut u64);
}

#[repr(C)]
struct OnceGuard {
    word: UnsafeCell<u64>,
}

// 该 guard 的并发写入由 runtime 内部的原子操作保证；对 Rust 来说属于“受控的 interior mutability”。
unsafe impl Send for OnceGuard {}
unsafe impl Sync for OnceGuard {}

#[test]
fn once_guard_runs_init_at_most_once_under_threads() {
    let guard = Arc::new(OnceGuard {
        word: UnsafeCell::new(0),
    });

    let init_count = Arc::new(AtomicU64::new(0));

    // 用 barrier 同步起跑，制造竞争。
    let n = 8usize;
    let start = Arc::new(Barrier::new(n));

    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let guard = Arc::clone(&guard);
        let init_count = Arc::clone(&init_count);
        let start = Arc::clone(&start);

        handles.push(thread::spawn(move || {
            start.wait();

            let guard_ptr = guard.word.get();
            unsafe {
                let should_init = scoop_once_begin(guard_ptr);
                if should_init != 0 {
                    init_count.fetch_add(1, Ordering::SeqCst);

                    // 重入：同线程在 initializing 阶段再次 begin 不应死锁。
                    assert_eq!(scoop_once_begin(guard_ptr), 0);

                    // 留出时间让其它线程进入 begin 并走等待路径。
                    thread::sleep(Duration::from_millis(10));

                    scoop_once_end(guard_ptr);
                }
            }
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }

    assert_eq!(init_count.load(Ordering::SeqCst), 1);
}
