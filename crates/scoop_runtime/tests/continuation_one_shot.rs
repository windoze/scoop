// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicU32, AtomicUsize, Ordering};

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

struct HandlerSnapshotObservations {
    found_ptr: AtomicUsize,
    found_op_tag: AtomicU32,
    found_active: AtomicU32,
}

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_effect_is_active() -> u32;
    fn scoop_effect_clear();
    fn scoop_effect_perform_slot_read_op_tag() -> u32;
    fn scoop_effect_perform_slot_read_effect_instance_key() -> u32;
    fn scoop_effect_perform_slot_read_len_words() -> u32;
    fn scoop_effect_perform_slot_read_gc_ref() -> *mut c_void;
    fn scoop_effect_perform_slot_read_u64() -> u64;

    fn scoop_callee_suspend_state_get() -> *mut c_void;
    fn scoop_test_callee_suspend_state_set(state: *mut c_void);
    fn scoop_test_continuation_resume_replay_state_create(
        prev_callee_suspend_state: *mut c_void,
    ) -> *mut c_void;

    fn scoop_effect_handler_stack_push(frame: *mut ScoopEffectHandlerFrame, op_tag: u32);
    fn scoop_effect_handler_stack_pop(frame: *mut ScoopEffectHandlerFrame);
    fn scoop_effect_handler_stack_find_nearest(op_tag: u32) -> *mut ScoopEffectHandlerFrame;
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

extern "C" fn replace_callee_suspend_state_step(
    state: *mut c_void,
    _resume_word: u64,
    _resume_gc_ref: *mut c_void,
) {
    unsafe {
        OBSERVED_CALLEE_SUSPEND_STATE.store(scoop_callee_suspend_state_get(), Ordering::SeqCst);
        scoop_test_callee_suspend_state_set(state);
    }
}

extern "C" fn observe_handler_snapshot_step(
    state: *mut c_void,
    _resume_word: u64,
    _resume_gc_ref: *mut c_void,
) {
    if state.is_null() {
        return;
    }

    let observations = unsafe { &*(state as *const HandlerSnapshotObservations) };
    let found = unsafe { scoop_effect_handler_stack_find_nearest(42) };
    observations
        .found_ptr
        .store(found as usize, Ordering::SeqCst);
    if found.is_null() {
        observations.found_op_tag.store(0, Ordering::SeqCst);
        observations.found_active.store(0, Ordering::SeqCst);
        return;
    }

    let found_ref = unsafe { &*found };
    observations
        .found_op_tag
        .store(found_ref.op_tag, Ordering::SeqCst);
    observations
        .found_active
        .store(found_ref.active, Ordering::SeqCst);
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
        let captured = prefix.captured_handler_stack_top;
        assert!(
            !captured.is_null(),
            "continuation must keep a non-null handler snapshot when a handler is active"
        );
        assert_eq!(
            (*captured).op_tag,
            42,
            "captured handler snapshot must preserve the active op_tag"
        );
        assert_eq!(
            (*captured).active,
            1,
            "captured handler snapshot must stay active for future redispatch"
        );
        assert_eq!(
            (*captured).prev,
            ptr::null_mut(),
            "single-frame handler stack should clone to a single-frame snapshot"
        );
        assert_ne!(
            captured, top,
            "continuation must not keep borrowing the original stack-allocated handler frame"
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
fn continuation_double_resume_uses_shared_runtime_error_transport_contract() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
        scoop_effect_clear();

        let k = scoop_continuation_alloc(ptr::null_mut(), Some(noop_step));
        assert!(
            !k.is_null(),
            "scoop_continuation_alloc must return non-null"
        );

        scoop_continuation_resume(k);
        scoop_effect_clear();

        scoop_continuation_resume(k);

        assert_eq!(
            scoop_effect_is_active(),
            1,
            "double resume should publish Raise<RuntimeError> through the active TLS flag"
        );
        assert_eq!(
            scoop_effect_perform_slot_read_op_tag(),
            1,
            "double resume should raise through the canonical Raise.raise op_tag"
        );
        assert_eq!(
            scoop_effect_perform_slot_read_effect_instance_key(),
            u32::MAX,
            "double resume should use the dedicated Raise<RuntimeError> effect instance key"
        );
        assert_eq!(
            scoop_effect_perform_slot_read_len_words(),
            1,
            "RuntimeError unit variants must travel through the shared single-word transport"
        );
        assert!(
            scoop_effect_perform_slot_read_gc_ref().is_null(),
            "ContinuationAlreadyResumed is a unit RuntimeError variant and must not publish a gc_ref payload"
        );
        assert_eq!(
            scoop_effect_perform_slot_read_u64(),
            2,
            "double resume should transport the concrete ContinuationAlreadyResumed variant tag"
        );

        scoop_effect_clear();
        scoop_thread_unregister();
    }
}

#[test]
fn continuation_resume_keeps_captured_handler_snapshot_alive_after_original_frame_pops() {
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

        let observations = Box::new(HandlerSnapshotObservations {
            found_ptr: AtomicUsize::new(0),
            found_op_tag: AtomicU32::new(0),
            found_active: AtomicU32::new(0),
        });
        let observations_ptr = Box::into_raw(observations) as *mut c_void;

        let k = scoop_continuation_alloc(observations_ptr, Some(observe_handler_snapshot_step));
        assert!(
            !k.is_null(),
            "scoop_continuation_alloc must return non-null"
        );

        scoop_effect_handler_stack_pop(&mut frame);
        assert_eq!(
            scoop_effect_handler_stack_top(),
            ptr::null_mut(),
            "popping the original handler must clear the caller TLS stack"
        );

        scoop_continuation_resume(k);

        assert_eq!(
            scoop_effect_handler_stack_top(),
            ptr::null_mut(),
            "resume must restore the caller TLS handler stack after step_fn returns"
        );

        let observations = Box::from_raw(observations_ptr as *mut HandlerSnapshotObservations);
        assert_ne!(
            observations.found_ptr.load(Ordering::SeqCst),
            0,
            "resumed continuation must still observe a matching handler frame"
        );
        assert_ne!(
            observations.found_ptr.load(Ordering::SeqCst),
            (&mut frame as *mut ScoopEffectHandlerFrame) as usize,
            "matching handler must come from the continuation snapshot, not the popped original frame"
        );
        assert_eq!(
            observations.found_op_tag.load(Ordering::SeqCst),
            42,
            "resumed continuation must redispatch through the captured handler op_tag"
        );
        assert_eq!(
            observations.found_active.load(Ordering::SeqCst),
            1,
            "captured handler snapshot must remain active during resumed execution"
        );

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

#[test]
fn continuation_resume_preserves_step_fn_replaced_callee_suspend_state() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        let saved_tls_state = scoop_sync_once_create();
        let captured_state = scoop_sync_once_create();
        let replacement_state = scoop_sync_once_create();
        assert!(
            !saved_tls_state.is_null(),
            "saved TLS sentinel must be allocated"
        );
        assert!(
            !captured_state.is_null(),
            "captured callee suspend sentinel must be allocated"
        );
        assert!(
            !replacement_state.is_null(),
            "replacement callee suspend sentinel must be allocated"
        );

        scoop_test_callee_suspend_state_set(saved_tls_state);
        OBSERVED_CALLEE_SUSPEND_STATE.store(ptr::null_mut(), Ordering::SeqCst);

        let k =
            scoop_continuation_alloc(replacement_state, Some(replace_callee_suspend_state_step));
        assert!(
            !k.is_null(),
            "scoop_continuation_alloc must return non-null"
        );

        let cont = &mut *(k as *mut ScoopContinuation);
        cont.captured_callee_suspend_state = captured_state;

        scoop_continuation_resume(k);

        assert_eq!(
            OBSERVED_CALLEE_SUSPEND_STATE.load(Ordering::SeqCst),
            captured_state,
            "step_fn must still start with the continuation-captured callee suspend state in TLS"
        );
        assert_eq!(
            scoop_callee_suspend_state_get(),
            replacement_state,
            "resume_common must preserve the TLS value that step_fn replaced, instead of resurrecting the caller's stale saved state"
        );

        scoop_thread_unregister();
    }
}

#[test]
fn continuation_resume_does_not_resurrect_saved_replay_state_tls() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        let replay_state = scoop_test_continuation_resume_replay_state_create(ptr::null_mut());
        assert!(
            !replay_state.is_null(),
            "replay-state sentinel must be allocated"
        );

        scoop_test_callee_suspend_state_set(replay_state);

        let k = scoop_continuation_alloc(ptr::null_mut(), Some(noop_step));
        assert!(
            !k.is_null(),
            "scoop_continuation_alloc must return non-null"
        );

        scoop_continuation_resume(k);

        assert_eq!(
            scoop_callee_suspend_state_get(),
            ptr::null_mut(),
            "resume_common must treat saved continuation-resume replay-state as one-shot bookkeeping instead of restoring it into TLS after the child continuation finishes"
        );

        scoop_thread_unregister();
    }
}
