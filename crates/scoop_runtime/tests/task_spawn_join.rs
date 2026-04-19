// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;

unsafe extern "C" {
    fn scoop_task_from_result(result_word: u64, result_gc_ref: *mut c_void) -> *mut c_void;
    fn scoop_task_join(task_obj: *mut c_void, out_gc_ref: *mut *mut c_void) -> u64;
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
