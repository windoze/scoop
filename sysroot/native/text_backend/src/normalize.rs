use std::ptr;

use icu_normalizer::ComposingNormalizerBorrowed;

use crate::bytes;

#[repr(C)]
struct CharsetText {
    data: *mut u8,
    size: i64,
    code: i32,
    backend: i32,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_normalize_nfc(data: *const u8, size: i64) -> CharsetText {
    // SAFETY: The C caller keeps the input span readable during this call.
    match unsafe { bytes::input(data, size) } {
        Ok(text) => {
            let normalized = ComposingNormalizerBorrowed::new_nfc().normalize(text);
            let (data, size) = bytes::owned(normalized.into_owned());
            CharsetText {
                data,
                size,
                code: 0,
                backend: 0,
            }
        }
        Err(_) => CharsetText {
            data: ptr::null_mut(),
            size: 0,
            code: 9,
            backend: 0,
        },
    }
}
