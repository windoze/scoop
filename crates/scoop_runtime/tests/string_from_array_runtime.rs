use scoop_runtime as _;

use core::ffi::c_void;
use core::mem;
use core::ptr;
use core::slice;

const ARRAY_ELEM_KIND_WORD: u32 = 1;
const ARRAY_ELEM_KIND_REF: u32 = 2;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

#[repr(C)]
struct ScoopString {
    hdr: ScoopGcObjectHeader,
    len: u64,
    data: *const u8,
}

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_mutable_array_new(
        elem_kind: u32,
        elem_size: u64,
        elem_align: u64,
        elem_desc: *const c_void,
        capacity: u64,
    ) -> *mut c_void;
    fn scoop_mutable_array_push_word(arr: *mut c_void, value: u64);
    fn scoop_mutable_array_push_ref(arr: *mut c_void, value: *mut c_void);

    fn scoop_string_from_byte_array(bytes: *mut c_void) -> *const ScoopString;
    fn scoop_string_from_char_array(chars: *mut c_void) -> *const ScoopString;
    fn scoop_string_from_string_array(parts: *mut c_void) -> *const ScoopString;
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

fn word_array(elem_size: u64, elem_align: u64, values: &[u64]) -> *mut c_void {
    unsafe {
        let arr = scoop_mutable_array_new(
            ARRAY_ELEM_KIND_WORD,
            elem_size,
            elem_align,
            ptr::null(),
            values.len() as u64,
        );
        assert!(!arr.is_null());
        for value in values {
            scoop_mutable_array_push_word(arr, *value);
        }
        arr
    }
}

fn byte_array(bytes: &[u8]) -> *mut c_void {
    let values: Vec<u64> = bytes.iter().map(|byte| u64::from(*byte)).collect();
    word_array(1, 1, &values)
}

fn char_array(chars: &[u32]) -> *mut c_void {
    let values: Vec<u64> = chars
        .iter()
        .map(|codepoint| u64::from(*codepoint))
        .collect();
    word_array(4, 4, &values)
}

fn ref_array(values: &[*const ScoopString]) -> *mut c_void {
    unsafe {
        let arr = scoop_mutable_array_new(
            ARRAY_ELEM_KIND_REF,
            mem::size_of::<usize>() as u64,
            mem::align_of::<usize>() as u64,
            ptr::null(),
            values.len() as u64,
        );
        assert!(!arr.is_null());
        for value in values {
            scoop_mutable_array_push_ref(arr, *value as *mut c_void);
        }
        arr
    }
}

fn string_bytes(value: *const ScoopString) -> Vec<u8> {
    unsafe {
        assert!(!value.is_null());
        let len = (*value).len as usize;
        if len == 0 {
            return Vec::new();
        }
        assert!(!(*value).data.is_null());
        slice::from_raw_parts((*value).data, len).to_vec()
    }
}

#[test]
fn string_from_byte_array_basic() {
    let _thread = RuntimeThread::enter();

    let bytes = byte_array(b"hello");
    let result = unsafe { scoop_string_from_byte_array(bytes) };

    assert_eq!(string_bytes(result), b"hello");
}

#[test]
fn string_from_byte_array_preserves_unchecked_bytes() {
    let _thread = RuntimeThread::enter();

    let bytes = byte_array(&[b'a', 0xff, b'z']);
    let result = unsafe { scoop_string_from_byte_array(bytes) };

    assert_eq!(string_bytes(result), [b'a', 0xff, b'z']);
}

#[test]
fn string_from_char_array_handles_4byte_codepoint() {
    let _thread = RuntimeThread::enter();

    let chars = char_array(&[u32::from('A'), 0x1f600]);
    let result = unsafe { scoop_string_from_char_array(chars) };

    assert_eq!(string_bytes(result), "A😀".as_bytes());
}

#[test]
fn string_from_char_array_replaces_surrogate_with_replacement_char() {
    let _thread = RuntimeThread::enter();

    let chars = char_array(&[0xd800]);
    let result = unsafe { scoop_string_from_char_array(chars) };

    assert_eq!(string_bytes(result), "�".as_bytes());
}

#[test]
fn string_from_string_array_concatenates_correct_total_length() {
    let _thread = RuntimeThread::enter();

    let hello = unsafe { scoop_string_from_byte_array(byte_array(b"hello")) };
    let space = unsafe { scoop_string_from_byte_array(byte_array(b" ")) };
    let face = unsafe { scoop_string_from_char_array(char_array(&[0x1f600])) };
    let parts = ref_array(&[hello, space, face]);

    let result = unsafe { scoop_string_from_string_array(parts) };

    assert_eq!(string_bytes(result), "hello 😀".as_bytes());
}

#[test]
fn string_from_empty_arrays_return_empty_string() {
    let _thread = RuntimeThread::enter();

    let byte_result = unsafe { scoop_string_from_byte_array(byte_array(&[])) };
    let char_result = unsafe { scoop_string_from_char_array(char_array(&[])) };
    let string_result = unsafe { scoop_string_from_string_array(ref_array(&[])) };

    assert_eq!(string_bytes(byte_result), b"");
    assert_eq!(string_bytes(char_result), b"");
    assert_eq!(string_bytes(string_result), b"");
}
