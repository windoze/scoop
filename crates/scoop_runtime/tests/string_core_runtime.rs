use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;
use core::slice;

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_char_to_string(value: i32) -> *const c_void;
    fn scoop_string_byte_length(value: *const c_void) -> u64;
    fn scoop_string_bytes(value: *const c_void) -> *const u8;
    fn scoop_string_from_owned_bytes(bytes: *mut u8, len: u64) -> *const c_void;
}

struct RuntimeThread;

impl RuntimeThread {
    fn enter() -> Self {
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
        }
        Self
    }
}

impl Drop for RuntimeThread {
    fn drop(&mut self) {
        unsafe {
            scoop_thread_unregister();
        }
    }
}

fn owned_runtime_bytes(bytes: &[u8]) -> *mut u8 {
    if bytes.is_empty() {
        return ptr::null_mut();
    }
    unsafe {
        let out = libc::malloc(bytes.len()) as *mut u8;
        assert!(!out.is_null());
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        out
    }
}

fn string_bytes(value: *const c_void) -> Vec<u8> {
    unsafe {
        let len = scoop_string_byte_length(value) as usize;
        let data = scoop_string_bytes(value);
        if len == 0 {
            assert!(data.is_null());
            return Vec::new();
        }
        assert!(!data.is_null());
        slice::from_raw_parts(data, len).to_vec()
    }
}

#[test]
fn string_owned_bytes_substrate_preserves_bytes() {
    let _thread = RuntimeThread::enter();

    let value =
        unsafe { scoop_string_from_owned_bytes(owned_runtime_bytes(&[b'a', 0xff, b'z']), 3) };

    assert_eq!(string_bytes(value), [b'a', 0xff, b'z']);
}

#[test]
fn string_owned_bytes_substrate_handles_empty() {
    let _thread = RuntimeThread::enter();

    let value = unsafe { scoop_string_from_owned_bytes(ptr::null_mut(), 0) };

    assert_eq!(string_bytes(value), b"");
}

#[test]
fn string_accessors_cover_core_char_to_string() {
    let _thread = RuntimeThread::enter();

    let value = unsafe { scoop_char_to_string(0x1f600) };

    assert_eq!(string_bytes(value), "😀".as_bytes());
}
