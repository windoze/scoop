// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

// 对齐 `runtime/c/scoop_runtime.c` 的 `ScoopEffectHandlerFrame`（TODO T0913）。
#[repr(C)]
struct ScoopEffectHandlerFrame {
    prev: *mut ScoopEffectHandlerFrame,
    op_tag: u32,
    active: u32,
}

type ScoopContinuationStepFn =
    Option<extern "C" fn(state: *mut c_void, resume_word: u64, resume_gc_ref: *mut c_void)>;

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_effect_handler_stack_push(frame: *mut ScoopEffectHandlerFrame, op_tag: u32);
    fn scoop_effect_handler_stack_pop(frame: *mut ScoopEffectHandlerFrame);
    fn scoop_effect_handler_stack_top() -> *mut ScoopEffectHandlerFrame;

    fn scoop_continuation_alloc(
        state: *mut c_void,
        step_fn: ScoopContinuationStepFn,
    ) -> *mut c_void;
    fn scoop_continuation_resume_u64(continuation: *mut c_void, resume_value: u64);
}

struct ResumeObservations {
    expected_original_top: *mut ScoopEffectHandlerFrame,
    observed_top: AtomicUsize,
    observed_op_tag: AtomicU32,
    observed_active: AtomicU32,
    observed_value: AtomicU64,
}

extern "C" fn observe_step(state: *mut c_void, resume_value: u64, _resume_gc_ref: *mut c_void) {
    if state.is_null() {
        return;
    }

    let observations = unsafe { &*(state as *const ResumeObservations) };
    let top = unsafe { scoop_effect_handler_stack_top() };
    observations
        .observed_top
        .store(top as usize, Ordering::SeqCst);
    if top.is_null() {
        observations.observed_op_tag.store(0, Ordering::SeqCst);
        observations.observed_active.store(0, Ordering::SeqCst);
    } else {
        let top_ref = unsafe { &*top };
        observations
            .observed_op_tag
            .store(top_ref.op_tag, Ordering::SeqCst);
        observations
            .observed_active
            .store(top_ref.active, Ordering::SeqCst);
    }
    observations
        .observed_value
        .store(resume_value, Ordering::SeqCst);
}

#[test]
fn continuation_resume_swaps_handler_stack_across_threads_and_restores_after() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());
    }

    // 在主线程安装一个 handler frame，并用它来构造“被捕获的 handler stack”。
    let mut frame = ScoopEffectHandlerFrame {
        prev: ptr::null_mut(),
        op_tag: 0,
        active: 0,
    };

    let captured_top = unsafe {
        scoop_effect_handler_stack_push(&mut frame, 77);
        let top = scoop_effect_handler_stack_top();
        assert_eq!(top, &mut frame as *mut _);
        top
    };

    let observations = Box::new(ResumeObservations {
        expected_original_top: captured_top,
        observed_top: AtomicUsize::new(0),
        observed_op_tag: AtomicU32::new(0),
        observed_active: AtomicU32::new(0),
        observed_value: AtomicU64::new(0),
    });
    let observations_ptr = Box::into_raw(observations) as *mut c_void;

    let k = unsafe { scoop_continuation_alloc(observations_ptr, Some(observe_step)) };
    assert!(
        !k.is_null(),
        "scoop_continuation_alloc must return non-null"
    );
    let k_addr = k as usize;

    // 在另一个线程执行 `resume`：step_fn 观察到的 handler stack top 应当等于 captured 值；
    // 并且 `resume` 返回后，调用方线程 TLS 必须恢复为原值（这里为 null）。
    let join = std::thread::spawn(move || unsafe {
        let k = k_addr as *mut c_void;
        scoop_thread_register();
        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());

        scoop_continuation_resume_u64(k, 123);

        assert_eq!(
            scoop_effect_handler_stack_top(),
            ptr::null_mut(),
            "resume must restore caller TLS handler stack after step_fn returns"
        );
        scoop_thread_unregister();
    });

    join.join().expect("resume thread must not panic");

    // 主线程 TLS 不应受到其它线程的 resume 影响。
    unsafe {
        assert_eq!(scoop_effect_handler_stack_top(), captured_top);
    }

    let observations = unsafe { Box::from_raw(observations_ptr as *mut ResumeObservations) };
    assert_ne!(
        observations.observed_top.load(Ordering::SeqCst),
        0,
        "step_fn must observe a non-null captured handler snapshot"
    );
    assert_eq!(
        observations.observed_op_tag.load(Ordering::SeqCst),
        77,
        "step_fn must observe the captured handler op_tag"
    );
    assert_eq!(
        observations.observed_active.load(Ordering::SeqCst),
        1,
        "captured handler snapshot must stay active during resumed execution"
    );
    assert_ne!(
        observations.observed_top.load(Ordering::SeqCst),
        observations.expected_original_top as usize,
        "cross-thread resume must install a continuation-owned handler snapshot, not borrow the original stack frame"
    );
    assert_eq!(
        observations.observed_value.load(Ordering::SeqCst),
        123,
        "resume value must be forwarded to step_fn"
    );

    unsafe {
        scoop_effect_handler_stack_pop(&mut frame);
        scoop_thread_unregister();
    }
}
