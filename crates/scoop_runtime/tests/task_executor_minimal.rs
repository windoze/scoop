// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static EVENTS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static TASK_HANDLE: AtomicU64 = AtomicU64::new(0);

type ScoopTaskBodyU64Fn = Option<extern "C" fn(ctx: *mut c_void) -> u64>;
type ScoopContinuationStepFn = Option<extern "C" fn(state: *mut c_void, resume_value: u64)>;

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_continuation_alloc(
        state: *mut c_void,
        step_fn: ScoopContinuationStepFn,
    ) -> *mut c_void;

    fn scoop_executor_create() -> u64;
    fn scoop_executor_destroy(executor_handle: u64);
    fn scoop_executor_debug_pending_count(executor_handle: u64) -> u64;
    fn scoop_executor_run_until_idle(executor_handle: u64, max_steps: u64) -> u64;

    fn scoop_task_u64_create(body_fn: ScoopTaskBodyU64Fn, body_ctx: *mut c_void) -> u64;
    fn scoop_task_u64_destroy(task_handle: u64);
    fn scoop_task_u64_state(task_handle: u64) -> u32;
    fn scoop_task_u64_result(task_handle: u64) -> u64;
    fn scoop_task_u64_try_start(task_handle: u64, executor_handle: u64) -> u32;
    fn scoop_task_u64_on_complete_resume_u64(
        task_handle: u64,
        executor_handle: u64,
        continuation: *mut c_void,
    ) -> u32;
}

fn lock_events() -> std::sync::MutexGuard<'static, Vec<u64>> {
    match EVENTS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

extern "C" fn task_body_u64(_ctx: *mut c_void) -> u64 {
    lock_events().push(100);
    42
}

extern "C" fn cont_step_a(_state: *mut c_void, resume_value: u64) {
    let task = TASK_HANDLE.load(Ordering::Relaxed);
    let state = unsafe { scoop_task_u64_state(task) };

    let mut events = lock_events();
    events.push(1);
    events.push(resume_value);
    events.push(state as u64);
}

extern "C" fn cont_step_b(_state: *mut c_void, resume_value: u64) {
    let task = TASK_HANDLE.load(Ordering::Relaxed);
    let state = unsafe { scoop_task_u64_state(task) };

    let mut events = lock_events();
    events.push(2);
    events.push(resume_value);
    events.push(state as u64);
}

#[test]
fn task_executor_minimal_start_complete_and_resume_waiters_in_order() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
    }

    lock_events().clear();

    let executor = unsafe { scoop_executor_create() };
    assert_ne!(executor, 0);

    let task = unsafe { scoop_task_u64_create(Some(task_body_u64), ptr::null_mut()) };
    assert_ne!(task, 0);
    TASK_HANDLE.store(task, Ordering::Relaxed);

    // 0=created（见 runtime/c/scoop_runtime.c：SCOOP_TASK_STATE_CREATED）。
    assert_eq!(unsafe { scoop_task_u64_state(task) }, 0);
    assert_eq!(unsafe { scoop_executor_debug_pending_count(executor) }, 0);
    assert_eq!(unsafe { scoop_executor_run_until_idle(executor, 100) }, 0);
    assert!(lock_events().is_empty());

    // 注册两个等待者：要求按注册顺序恢复（回归“回调顺序稳定”）。
    let k1 = unsafe { scoop_continuation_alloc(ptr::null_mut(), Some(cont_step_a)) };
    assert!(!k1.is_null());
    let k2 = unsafe { scoop_continuation_alloc(ptr::null_mut(), Some(cont_step_b)) };
    assert!(!k2.is_null());

    assert_eq!(
        unsafe { scoop_task_u64_on_complete_resume_u64(task, executor, k1) },
        1
    );
    assert_eq!(
        unsafe { scoop_task_u64_on_complete_resume_u64(task, executor, k2) },
        1
    );

    // 显式 start：把 task body 入队到 executor。
    assert_eq!(unsafe { scoop_task_u64_try_start(task, executor) }, 1);
    // 1=scheduled
    assert_eq!(unsafe { scoop_task_u64_state(task) }, 1);
    assert_eq!(unsafe { scoop_executor_debug_pending_count(executor) }, 1);

    // run until idle：
    // - 先运行 task body（记录 100，并完成 task，入队两个 continuation）
    // - 再按顺序恢复 continuation（A 再 B）
    let ran = unsafe { scoop_executor_run_until_idle(executor, 100) };
    assert_eq!(ran, 3, "expected: run_task + resume(A) + resume(B)");

    // 3=completed
    assert_eq!(unsafe { scoop_task_u64_state(task) }, 3);
    assert_eq!(unsafe { scoop_task_u64_result(task) }, 42);
    assert_eq!(unsafe { scoop_executor_debug_pending_count(executor) }, 0);

    let events = lock_events().clone();
    assert_eq!(
        events,
        vec![
            100, // task body ran
            1, 42, 3, // waiter A resumed after completion
            2, 42, 3, // waiter B resumed after completion
        ]
    );

    unsafe {
        scoop_task_u64_destroy(task);
        scoop_executor_destroy(executor);
        scoop_thread_unregister();
    }
}
