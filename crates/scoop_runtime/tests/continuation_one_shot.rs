// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

// 对齐 `runtime/c/scoop_runtime.c` 的 `ScoopEffectHandlerFrame`（TODO T0913）。
#[repr(C)]
struct ScoopEffectHandlerFrame {
    prev: *mut ScoopEffectHandlerFrame,
    op_tag: u32,
    active: u32,
}

// 对齐 `runtime/c/scoop_gc.h` 的对象头（用于在测试中读取 continuation 捕获字段）。
#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

// 对齐 `runtime/c/scoop_runtime.c` 的 `ScoopContinuation` 布局。
#[repr(C)]
struct ScoopContinuation {
    hdr: ScoopGcObjectHeader,
    resumed: u32,
    resume_state_tag: u32,
    captured_handler_stack_top: *mut ScoopEffectHandlerFrame,
    state: *mut c_void,
    step_fn: ScoopContinuationStepFn,
    resume_word: u64,
    resume_gc_ref: *mut c_void,
    captured_callee_suspend_state: *mut c_void,
}

type ScoopContinuationStepFn =
    Option<extern "C" fn(state: *mut c_void, resume_word: u64, resume_gc_ref: *mut c_void)>;

static OBSERVED_CALLEE_SUSPEND_STATE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_callee_suspend_state_get() -> *mut c_void;
    fn scoop_test_callee_suspend_state_set(state: *mut c_void);

    fn scoop_effect_handler_stack_push(frame: *mut ScoopEffectHandlerFrame, op_tag: u32);
    fn scoop_effect_handler_stack_pop(frame: *mut ScoopEffectHandlerFrame);
    fn scoop_effect_handler_stack_top() -> *mut ScoopEffectHandlerFrame;

    fn scoop_continuation_alloc(
        state: *mut c_void,
        step_fn: ScoopContinuationStepFn,
    ) -> *mut c_void;
    fn scoop_continuation_try_resume(continuation: *mut c_void) -> u32;
    fn scoop_continuation_resume(continuation: *mut c_void);

    fn scoop_sync_once_create() -> *mut c_void;
}

extern "C" fn noop_step(_state: *mut c_void, _resume_word: u64, _resume_gc_ref: *mut c_void) {}

extern "C" fn observe_callee_suspend_state_step(
    _state: *mut c_void,
    _resume_word: u64,
    _resume_gc_ref: *mut c_void,
) {
    unsafe {
        OBSERVED_CALLEE_SUSPEND_STATE.store(scoop_callee_suspend_state_get(), Ordering::SeqCst);
    }
}

#[test]
fn continuation_alloc_captures_handler_stack_and_is_one_shot() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());

        let mut frame = ScoopEffectHandlerFrame {
            prev: ptr::null_mut(),
            op_tag: 0,
            active: 0,
        };
        scoop_effect_handler_stack_push(&mut frame, 42);
        let top = scoop_effect_handler_stack_top();
        assert_eq!(top, &mut frame as *mut _);

        let k = scoop_continuation_alloc(ptr::null_mut(), Some(noop_step));
        assert!(
            !k.is_null(),
            "scoop_continuation_alloc must return non-null"
        );

        let prefix = &*(k as *const ScoopContinuation);
        assert_eq!(
            prefix.captured_handler_stack_top, top,
            "continuation must capture handler stack top at suspension point"
        );

        // one-shot：第一次成功，第二次必须失败（spec §5.5：runtime error）。
        assert_eq!(scoop_continuation_try_resume(k), 1);
        assert_eq!(prefix.resumed, 1);
        assert_eq!(scoop_continuation_try_resume(k), 0);

        scoop_effect_handler_stack_pop(&mut frame);
        scoop_thread_unregister();
    }
}

#[test]
fn continuation_resume_temporarily_restores_captured_callee_suspend_state() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        let saved_tls_state = scoop_sync_once_create();
        let captured_state = scoop_sync_once_create();
        assert!(
            !saved_tls_state.is_null(),
            "saved TLS sentinel must be allocated"
        );
        assert!(
            !captured_state.is_null(),
            "captured callee suspend sentinel must be allocated"
        );

        scoop_test_callee_suspend_state_set(saved_tls_state);
        OBSERVED_CALLEE_SUSPEND_STATE.store(ptr::null_mut(), Ordering::SeqCst);

        let k = scoop_continuation_alloc(ptr::null_mut(), Some(observe_callee_suspend_state_step));
        assert!(
            !k.is_null(),
            "scoop_continuation_alloc must return non-null"
        );

        // 编译器在 suspend terminator 中会写这个字段；测试里直接模拟该 ABI 合同。
        let cont = &mut *(k as *mut ScoopContinuation);
        cont.captured_callee_suspend_state = captured_state;

        scoop_continuation_resume(k);

        assert_eq!(
            OBSERVED_CALLEE_SUSPEND_STATE.load(Ordering::SeqCst),
            captured_state,
            "step_fn must observe the continuation-captured callee suspend state via TLS"
        );
        assert_eq!(
            scoop_callee_suspend_state_get(),
            saved_tls_state,
            "resume_common must restore the caller's prior TLS callee suspend state after step_fn"
        );

        scoop_thread_unregister();
    }
}
