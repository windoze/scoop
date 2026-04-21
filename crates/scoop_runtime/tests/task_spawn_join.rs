// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

#[repr(C)]
struct ScoopEffectFrameResultPrefix {
    hdr: ScoopGcObjectHeader,
    state_tag: u32,
    _padding: u32,
    resume_word: u64,
    resume_gc_ref: *mut c_void,
}

type ScoopContinuationStepFn =
    Option<extern "C" fn(state: *mut c_void, resume_word: u64, resume_gc_ref: *mut c_void)>;

const SCOOP_EFFECT_FRAME_STATE_TAG_HANDLE_RETURNED: u32 = 0xFFFF_FFFE;

unsafe extern "C" {
    fn scoop_alloc(size: u64) -> *mut c_void;
    fn scoop_continuation_alloc(
        state: *mut c_void,
        step_fn: ScoopContinuationStepFn,
    ) -> *mut c_void;
    fn scoop_task_create(
        body_fn: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        body_closure_obj: *mut c_void,
    ) -> *mut c_void;
    fn scoop_task_from_result(result_word: u64, result_gc_ref: *mut c_void) -> *mut c_void;
    fn scoop_task_join(task_obj: *mut c_void, out_gc_ref: *mut *mut c_void) -> u64;
    fn scoop_task_poll(
        task_obj: *mut c_void,
        out_word: *mut u64,
        out_gc_ref: *mut *mut c_void,
    ) -> u32;
    fn scoop_task_step_pending(awaited_task: *mut c_void, continuation: *mut c_void)
    -> *mut c_void;
    fn scoop_task_step_ready(result_word: u64, result_gc_ref: *mut c_void) -> *mut c_void;
}

unsafe extern "C" fn task_body_returns_seven(_closure_obj: *mut c_void) -> *mut c_void {
    unsafe { scoop_task_step_ready(7, ptr::null_mut()) }
}

extern "C" fn resume_pending_task_step(
    state: *mut c_void,
    resume_word: u64,
    _resume_gc_ref: *mut c_void,
) {
    if state.is_null() {
        return;
    }

    let ready_step = unsafe { scoop_task_step_ready(resume_word + 1, ptr::null_mut()) };
    assert!(
        !ready_step.is_null(),
        "scoop_task_step_ready must allocate a step-result carrier"
    );

    let frame = unsafe { &mut *(state as *mut ScoopEffectFrameResultPrefix) };
    frame.state_tag = SCOOP_EFFECT_FRAME_STATE_TAG_HANDLE_RETURNED;
    frame.resume_word = 0;
    frame.resume_gc_ref = ready_step;
}

unsafe extern "C" fn task_body_returns_pending_then_ready(
    _closure_obj: *mut c_void,
) -> *mut c_void {
    let awaited_task = unsafe { scoop_task_from_result(41, ptr::null_mut()) };
    assert!(
        !awaited_task.is_null(),
        "scoop_task_from_result must allocate the awaited task"
    );

    let frame = unsafe {
        scoop_alloc(core::mem::size_of::<ScoopEffectFrameResultPrefix>() as u64)
            as *mut ScoopEffectFrameResultPrefix
    };
    assert!(
        !frame.is_null(),
        "suspend state frame allocation must succeed"
    );
    unsafe {
        (*frame).state_tag = 0;
        (*frame)._padding = 0;
        (*frame).resume_word = 0;
        (*frame).resume_gc_ref = ptr::null_mut();
    }

    let continuation =
        unsafe { scoop_continuation_alloc(frame as *mut c_void, Some(resume_pending_task_step)) };
    assert!(
        !continuation.is_null(),
        "scoop_continuation_alloc must return a non-null continuation"
    );

    unsafe { scoop_task_step_pending(awaited_task, continuation) }
}

#[test]
fn task_from_result_join_int_roundtrip() {
    unsafe {
        let task = scoop_task_from_result(42, ptr::null_mut());
        assert_ne!(task, ptr::null_mut());

        let mut gc_ref: *mut c_void = ptr::null_mut();
        let value = scoop_task_join(task, &mut gc_ref);
        assert_eq!(value, 42);
        assert_eq!(gc_ref, ptr::null_mut());
    }
}

#[test]
fn task_join_runs_created_task_body_inline() {
    unsafe {
        let task = scoop_task_create(task_body_returns_seven, ptr::null_mut());
        assert_ne!(task, ptr::null_mut());

        let mut gc_ref: *mut c_void = ptr::null_mut();
        let value = scoop_task_join(task, &mut gc_ref);
        assert_eq!(value, 7);
        assert_eq!(gc_ref, ptr::null_mut());
    }
}

#[test]
fn task_poll_runs_created_task_body_until_ready() {
    unsafe {
        let task = scoop_task_create(task_body_returns_seven, ptr::null_mut());
        assert_ne!(task, ptr::null_mut());

        let mut word = 0_u64;
        let mut gc_ref: *mut c_void = ptr::null_mut();
        let ready = scoop_task_poll(task, &mut word, &mut gc_ref);
        assert_eq!(ready, 1);
        assert_eq!(word, 7);
        assert_eq!(gc_ref, ptr::null_mut());
    }
}

#[test]
fn task_poll_reads_completed_task_result() {
    unsafe {
        let task = scoop_task_from_result(42, ptr::null_mut());
        assert_ne!(task, ptr::null_mut());

        let mut word = 0_u64;
        let mut gc_ref: *mut c_void = ptr::null_mut();
        let ready = scoop_task_poll(task, &mut word, &mut gc_ref);
        assert_eq!(ready, 1);
        assert_eq!(word, 42);
        assert_eq!(gc_ref, ptr::null_mut());
    }
}

#[test]
fn task_poll_resumes_pending_task_via_shared_continuation_helper() {
    unsafe {
        let task = scoop_task_create(task_body_returns_pending_then_ready, ptr::null_mut());
        assert_ne!(task, ptr::null_mut());

        let mut word = 0_u64;
        let mut gc_ref: *mut c_void = ptr::null_mut();
        let ready = scoop_task_poll(task, &mut word, &mut gc_ref);
        assert_eq!(ready, 1);
        assert_eq!(word, 42);
        assert_eq!(gc_ref, ptr::null_mut());
    }
}
