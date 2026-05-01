// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

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
    state_handle: u64,
    step_fn: ScoopContinuationStepFn,
    resume_word: u64,
    resume_gc_ref: *mut c_void,
    captured_callee_suspend_state_handle: u64,
}

// GC-managed wrapper：把 Rust 堆上的观测数据指针“装箱”到 runtime GC heap 中。
//
// 说明：`scoop_continuation_alloc` 的 `state` 参数在 LLVM 侧被当作 GC ref（addrspace(1)）；
// 因此测试不能直接把 Rust `Box` 指针当作 state 传入。
#[repr(C)]
struct ContinuationStateWrapper {
    hdr: ScoopGcObjectHeader,
    observations: *mut c_void,
}

#[repr(C)]
struct ScoopEffectFrameResultPrefix {
    hdr: ScoopGcObjectHeader,
    state_tag: u32,
    _padding: u32,
    resume_word: u64,
    resume_gc_ref: *mut c_void,
}

#[repr(C)]
struct ScoopValueTransport {
    word: u64,
    gc_ref: *mut c_void,
}

#[repr(C)]
struct ScoopEffectSignal {
    op_tag: u32,
    effect_instance_key: u32,
    payload: ScoopValueTransport,
    resume_token: *mut c_void,
}

#[repr(C)]
struct ScoopEffectOutcome {
    tag: u32,
    reserved_u32: u32,
    complete: ScoopValueTransport,
    signal: ScoopEffectSignal,
}

type ScoopContinuationStepFn =
    Option<extern "C" fn(state: *mut c_void, resume_word: u64, resume_gc_ref: *mut c_void)>;

const SCOOP_EFFECT_FRAME_STATE_TAG_HANDLE_RETURNED: u32 = 0xFFFF_FFFE;
const SCOOP_EFFECT_OUTCOME_COMPLETE: u32 = 0;
const SCOOP_EFFECT_OUTCOME_PROPAGATE: u32 = 1;

static OBSERVED_CALLEE_SUSPEND_STATE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static CONTINUATION_TEST_LOCK: Mutex<()> = Mutex::new(());

struct HandlerSnapshotObservations {
    found_ptr: AtomicUsize,
    found_op_tag: AtomicU32,
    found_active: AtomicU32,
}

fn continuation_test_guard() -> MutexGuard<'static, ()> {
    CONTINUATION_TEST_LOCK
        .lock()
        .expect("continuation test lock")
}

unsafe extern "C" {
    fn scoop_alloc(size: u64) -> *mut c_void;

    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_effect_is_active() -> u32;
    fn scoop_effect_clear();
    fn scoop_effect_set_active();
    fn scoop_effect_perform_slot_write_u64(op_tag: u32, effect_instance_key: u32, value: u64);
    fn scoop_effect_perform_slot_read_op_tag() -> u32;
    fn scoop_effect_perform_slot_read_effect_instance_key() -> u32;
    fn scoop_effect_perform_slot_read_len_words() -> u32;
    fn scoop_effect_perform_slot_read_gc_ref() -> *mut c_void;
    fn scoop_effect_perform_slot_read_u64() -> u64;

    fn scoop_callee_suspend_state_get() -> *mut c_void;
    fn scoop_callee_suspend_state_clear();
    fn scoop_test_callee_suspend_state_set(state: *mut c_void);
    fn scoop_test_continuation_resume_replay_state_create(
        prev_callee_suspend_state: *mut c_void,
    ) -> *mut c_void;
    fn scoop_continuation_resume_publish_pending_continuation(continuation: *mut c_void);

    fn scoop_effect_handler_stack_push(frame: *mut ScoopEffectHandlerFrame, op_tag: u32);
    fn scoop_effect_handler_stack_pop(frame: *mut ScoopEffectHandlerFrame);
    fn scoop_effect_handler_stack_find_nearest(op_tag: u32) -> *mut ScoopEffectHandlerFrame;
    fn scoop_effect_handler_stack_top() -> *mut ScoopEffectHandlerFrame;

    fn scoop_continuation_alloc(
        state: *mut c_void,
        step_fn: ScoopContinuationStepFn,
    ) -> *mut c_void;
    fn scoop_continuation_set_captured_callee_suspend_state(
        continuation: *mut c_void,
        state: *mut c_void,
    );
    fn scoop_continuation_try_resume(continuation: *mut c_void) -> u32;
    fn scoop_continuation_resume(continuation: *mut c_void);
    fn scoop_continuation_resume_with(
        continuation: *mut c_void,
        resume_word: u64,
        resume_gc_ref: *mut c_void,
        out_word: *mut u64,
        out_gc_ref: *mut *mut c_void,
        out_effect_outcome: *mut ScoopEffectOutcome,
    ) -> u32;

    fn scoop_sync_once_create() -> *mut c_void;
}

extern "C" fn noop_step(_state: *mut c_void, _resume_word: u64, _resume_gc_ref: *mut c_void) {}

extern "C" fn write_answer_transport_step(
    state: *mut c_void,
    resume_word: u64,
    resume_gc_ref: *mut c_void,
) {
    if state.is_null() {
        return;
    }

    let frame = unsafe { &mut *(state as *mut ScoopEffectFrameResultPrefix) };
    frame.state_tag = SCOOP_EFFECT_FRAME_STATE_TAG_HANDLE_RETURNED;
    frame.resume_word = resume_word;
    frame.resume_gc_ref = resume_gc_ref;
}

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

extern "C" fn publish_pending_continuation_step(
    state: *mut c_void,
    _resume_word: u64,
    _resume_gc_ref: *mut c_void,
) {
    unsafe {
        scoop_continuation_resume_publish_pending_continuation(state);
    }
}

extern "C" fn publish_pending_continuation_with_active_effect_step(
    state: *mut c_void,
    _resume_word: u64,
    _resume_gc_ref: *mut c_void,
) {
    unsafe {
        scoop_continuation_resume_publish_pending_continuation(state);
        scoop_effect_perform_slot_write_u64(17, 29, 77);
        scoop_effect_set_active();
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

    // state 是 GC-managed wrapper；解包得到真实的 Rust 观测对象指针。
    let wrapper = unsafe { &*(state as *const ContinuationStateWrapper) };
    if wrapper.observations.is_null() {
        return;
    }
    let observations = unsafe { &*(wrapper.observations as *const HandlerSnapshotObservations) };

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
    let _guard = continuation_test_guard();
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
    let _guard = continuation_test_guard();
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
fn continuation_resume_with_returns_answer_transport_and_clears_outputs_on_failure() {
    let _guard = continuation_test_guard();
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
        scoop_effect_clear();

        let frame = scoop_alloc(core::mem::size_of::<ScoopEffectFrameResultPrefix>() as u64)
            as *mut ScoopEffectFrameResultPrefix;
        assert!(
            !frame.is_null(),
            "answer frame prefix allocation must succeed"
        );
        (*frame).state_tag = 0;
        (*frame)._padding = 0;
        (*frame).resume_word = 0;
        (*frame).resume_gc_ref = ptr::null_mut();

        let expected_gc_ref = scoop_sync_once_create();
        assert!(
            !expected_gc_ref.is_null(),
            "gc_ref answer payload sentinel must be allocated"
        );

        let k = scoop_continuation_alloc(frame as *mut c_void, Some(write_answer_transport_step));
        assert!(
            !k.is_null(),
            "scoop_continuation_alloc must return non-null"
        );

        let mut out_word = u64::MAX;
        let mut out_gc_ref = frame as *mut c_void;
        let mut outcome = ScoopEffectOutcome {
            tag: u32::MAX,
            reserved_u32: 0,
            complete: ScoopValueTransport {
                word: 0,
                gc_ref: ptr::null_mut(),
            },
            signal: ScoopEffectSignal {
                op_tag: 0,
                effect_instance_key: 0,
                payload: ScoopValueTransport {
                    word: 0,
                    gc_ref: ptr::null_mut(),
                },
                resume_token: ptr::null_mut(),
            },
        };
        assert_eq!(
            scoop_continuation_resume_with(
                k,
                77,
                expected_gc_ref,
                &mut out_word,
                &mut out_gc_ref,
                &mut outcome,
            ),
            1,
            "resume_with must report a delimiter answer when the resumed step finishes normally"
        );
        assert_eq!(out_word, 77);
        assert_eq!(out_gc_ref, expected_gc_ref);
        assert_eq!(outcome.tag, SCOOP_EFFECT_OUTCOME_COMPLETE);
        assert!(
            outcome.signal.resume_token.is_null(),
            "successful resume_with must not synthesize a replay token"
        );
        assert_eq!(
            scoop_effect_is_active(),
            0,
            "successful resume_with must not leave the effect-active flag set"
        );

        out_word = 999;
        out_gc_ref = expected_gc_ref;
        outcome.tag = u32::MAX;
        outcome.signal.resume_token = expected_gc_ref;
        assert_eq!(
            scoop_continuation_resume_with(
                k,
                123,
                ptr::null_mut(),
                &mut out_word,
                &mut out_gc_ref,
                &mut outcome,
            ),
            0,
            "double resume should report that no delimiter answer was produced"
        );
        assert_eq!(
            out_word, 0,
            "failed resume_with must clear the scalar out slot"
        );
        assert_eq!(
            out_gc_ref,
            ptr::null_mut(),
            "failed resume_with must clear the gc_ref out slot"
        );
        assert_eq!(
            outcome.tag, SCOOP_EFFECT_OUTCOME_PROPAGATE,
            "failed resume_with must surface RuntimeError through explicit effect outcome"
        );
        assert_eq!(outcome.signal.op_tag, 1);
        assert_eq!(outcome.signal.effect_instance_key, u32::MAX);
        assert_eq!(outcome.signal.payload.word, 2);
        assert!(
            outcome.signal.payload.gc_ref.is_null(),
            "RuntimeError unit variant transport should still carry no gc_ref"
        );
        assert!(
            outcome.signal.resume_token.is_null(),
            "double resume does not publish a replay token"
        );
        assert_eq!(
            scoop_effect_is_active(),
            0,
            "explicit outcome path should consume the runtime error instead of leaving TLS active"
        );

        scoop_effect_clear();
        scoop_thread_unregister();
    }
}

#[test]
fn continuation_resume_with_surfaces_pending_continuation_via_effect_outcome_resume_token() {
    let _guard = continuation_test_guard();
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
        scoop_effect_clear();
        scoop_callee_suspend_state_clear();

        let pending = scoop_continuation_alloc(ptr::null_mut(), Some(noop_step));
        assert!(
            !pending.is_null(),
            "pending continuation sentinel must be allocated"
        );

        let outer = scoop_continuation_alloc(
            pending,
            Some(publish_pending_continuation_with_active_effect_step),
        );
        assert!(!outer.is_null(), "outer continuation must be allocated");

        let mut out_word = u64::MAX;
        let mut out_gc_ref = pending;
        let mut outcome = ScoopEffectOutcome {
            tag: u32::MAX,
            reserved_u32: 0,
            complete: ScoopValueTransport {
                word: 0,
                gc_ref: ptr::null_mut(),
            },
            signal: ScoopEffectSignal {
                op_tag: 0,
                effect_instance_key: 0,
                payload: ScoopValueTransport {
                    word: 0,
                    gc_ref: ptr::null_mut(),
                },
                resume_token: ptr::null_mut(),
            },
        };

        assert_eq!(
            scoop_continuation_resume_with(
                outer,
                11,
                ptr::null_mut(),
                &mut out_word,
                &mut out_gc_ref,
                &mut outcome,
            ),
            0,
            "propagating resume_with should report that no delimiter answer was produced"
        );
        assert_eq!(out_word, 0);
        assert_eq!(out_gc_ref, ptr::null_mut());
        assert_eq!(outcome.tag, SCOOP_EFFECT_OUTCOME_PROPAGATE);
        assert_eq!(outcome.signal.op_tag, 17);
        assert_eq!(outcome.signal.effect_instance_key, 29);
        assert_eq!(outcome.signal.payload.word, 77);
        assert_eq!(
            outcome.signal.resume_token, pending,
            "pending inner continuation must travel through EffectSignal.resume_token"
        );
        assert_eq!(
            scoop_effect_is_active(),
            0,
            "explicit outcome path should consume the temporary TLS propagation state"
        );
        assert!(
            scoop_callee_suspend_state_get().is_null(),
            "resume_with outcome path must not package pending continuation into callee_suspend_state replay-state"
        );

        scoop_thread_unregister();
    }
}

#[test]
fn continuation_resume_keeps_captured_handler_snapshot_alive_after_original_frame_pops() {
    let _guard = continuation_test_guard();
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

        // 通过 runtime 分配 GC-managed state wrapper（见上方注释）。
        let state = scoop_alloc(core::mem::size_of::<ContinuationStateWrapper>() as u64);
        assert!(
            !state.is_null(),
            "continuation state wrapper must be allocated"
        );
        {
            let wrapper = &mut *(state as *mut ContinuationStateWrapper);
            wrapper.observations = observations_ptr;
        }

        let k = scoop_continuation_alloc(state, Some(observe_handler_snapshot_step));
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
    let _guard = continuation_test_guard();
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

        // 对齐 compiler 生成路径：通过 runtime helper 把 captured callee suspend state
        // 写入 continuation（内部会创建 stable handle）。
        scoop_continuation_set_captured_callee_suspend_state(k, captured_state);

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
    let _guard = continuation_test_guard();
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

        scoop_continuation_set_captured_callee_suspend_state(k, captured_state);

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
    let _guard = continuation_test_guard();
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

#[test]
fn continuation_publish_pending_continuation_is_scoped_to_active_resume_driver() {
    let _guard = continuation_test_guard();
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
        scoop_callee_suspend_state_clear();

        let pending = scoop_continuation_alloc(ptr::null_mut(), Some(noop_step));
        assert!(
            !pending.is_null(),
            "pending continuation sentinel must be allocated"
        );

        scoop_continuation_resume_publish_pending_continuation(pending);
        assert!(
            scoop_callee_suspend_state_get().is_null(),
            "publishing pending continuation outside an active resume scope must stay a no-op"
        );

        let outer = scoop_continuation_alloc(pending, Some(publish_pending_continuation_step));
        assert!(!outer.is_null(), "outer continuation must be allocated");

        scoop_continuation_resume(outer);

        let replay_state = scoop_callee_suspend_state_get();
        assert!(
            !replay_state.is_null(),
            "active resume scope should package the published pending continuation into replay state"
        );
        assert_ne!(
            replay_state, pending,
            "resume driver must not leak the raw pending continuation pointer as TLS source of truth"
        );

        scoop_thread_unregister();
    }
}
