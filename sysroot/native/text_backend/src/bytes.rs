use std::{ffi::c_char, ptr, slice, str};

pub const EMPTY: i32 = -1000;
pub const LABEL: i32 = -1001;
pub const BACKEND: i32 = -1002;
pub const DISALLOWED: i32 = -1003;
pub const LENGTH: i32 = -1004;
pub const PUNYCODE: i32 = -1005;
pub const INVALID: i32 = -1006;
pub const ARGUMENT: i32 = -1007;

#[derive(Clone, Copy)]
pub struct Failure {
    pub code: i32,
    pub label: i64,
}

impl Failure {
    pub const fn new(code: i32) -> Self {
        Self { code, label: -1 }
    }

    pub const fn at(code: i32, label: usize) -> Self {
        Self {
            code,
            label: label as i64,
        }
    }
}

#[repr(C)]
pub struct IdnaResult {
    pub data: *mut u8,
    pub size: i64,
    pub code: i32,
    pub label: i64,
}

impl IdnaResult {
    pub fn from_text(result: Result<String, Failure>) -> Self {
        match result {
            Ok(text) => {
                let (data, size) = owned(text);
                Self {
                    data,
                    size,
                    code: 0,
                    label: -1,
                }
            }
            Err(error) => Self {
                data: ptr::null_mut(),
                size: 0,
                code: error.code,
                label: error.label,
            },
        }
    }
}

/// The caller provides a readable span for this call, or null with zero size.
pub unsafe fn input<'a>(data: *const u8, size: i64) -> Result<&'a str, Failure> {
    let size = usize::try_from(size).map_err(|_| Failure::new(ARGUMENT))?;
    if size == 0 {
        return Ok("");
    }
    if size > isize::MAX as usize || data.is_null() {
        return Err(Failure::new(ARGUMENT));
    }
    // SAFETY: The caller owns the readable span until the synchronous call ends.
    let bytes = unsafe { slice::from_raw_parts(data, size) };
    str::from_utf8(bytes).map_err(|_| Failure::new(ARGUMENT))
}

pub fn owned(text: String) -> (*mut u8, i64) {
    if text.is_empty() {
        return (ptr::null_mut(), 0);
    }
    // Vec allocations cannot exceed isize::MAX, so this fits on all targets.
    let size = text.len() as i64;
    (
        Box::into_raw(text.into_bytes().into_boxed_slice()).cast(),
        size,
    )
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_text_free(data: *mut u8, size: i64) {
    if !data.is_null() {
        // SAFETY: The caller returns the exact pointer and size issued by owned,
        // at most once, after all reads have ended. Null represents empty output.
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                data,
                size as usize,
            )))
        };
    }
}

#[unsafe(no_mangle)]
extern "C" fn scoop_idna_message(code: i32) -> *const c_char {
    match code {
        EMPTY => c"empty domain name",
        LABEL => c"empty label or separator in a single label",
        DISALLOWED => c"domain contains a disallowed DNS hostname character",
        LENGTH => c"DNS label exceeds 63 bytes or wire name exceeds 255 bytes",
        PUNYCODE => c"invalid Punycode, integer overflow, or non-scalar value",
        INVALID => c"domain fails UTS #46 validation",
        ARGUMENT => c"invalid text backend argument",
        _ => c"text backend could not complete the operation",
    }
    .as_ptr()
}

#[unsafe(no_mangle)]
extern "C" fn scoop_idna_version() -> *const c_char {
    c"scoop_text_backend 0.1.0; idna 1.1.0; ICU4X 2.3.0".as_ptr()
}

#[unsafe(no_mangle)]
extern "C" fn scoop_text_unicode_version() -> *const c_char {
    // icu_{normalizer,properties}_data 2.3.0 use ICU release-78.1rc data.
    c"17.0.0".as_ptr()
}
