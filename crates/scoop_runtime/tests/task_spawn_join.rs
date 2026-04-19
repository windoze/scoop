// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;

unsafe extern "C" {
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
    fn scoop_task_step_ready(result_word: u64, result_gc_ref: *mut c_void) -> *mut c_void;
}

unsafe extern "C" fn task_body_returns_seven(_closure_obj: *mut c_void) -> *mut c_void {
    unsafe { scoop_task_step_ready(7, ptr::null_mut()) }
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
