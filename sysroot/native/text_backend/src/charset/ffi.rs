use alloc::{borrow::ToOwned, boxed::Box};
use core::{ffi::c_char, slice};

use super::{Engine, Options, Step, names};
use crate::bytes::{self, TextResult};

#[repr(C)]
struct Created {
    handle: u64,
    code: i32,
    backend: i32,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_name(data: *const u8, size: i64) -> TextResult {
    // SAFETY: Input is borrowed for the synchronous C call.
    let result = unsafe { bytes::input(data, size) }
        .map_err(|_| 9)
        .and_then(|name| {
            names::lookup(name)
                .map(|(name, _)| name.to_owned())
                .ok_or(1)
        });
    TextResult::from_text(result)
}

#[unsafe(no_mangle)]
extern "C" fn scoop_charset_names() -> TextResult {
    TextResult::from_text(Ok(names::names()))
}

#[unsafe(no_mangle)]
extern "C" fn scoop_charset_version() -> *const c_char {
    c"encoding_rs 0.8.42".as_ptr()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_new(
    source: *const u8,
    source_size: i64,
    destination: *const u8,
    destination_size: i64,
    options: Options,
) -> Created {
    // SAFETY: The C caller supplies readable name spans for this call.
    let result = (|| {
        let source = unsafe { bytes::input(source, source_size) }.map_err(|_| 9)?;
        let destination = unsafe { bytes::input(destination, destination_size) }.map_err(|_| 9)?;
        let (_, source) = names::lookup(source).ok_or(1)?;
        let (_, destination) = names::lookup(destination).ok_or(1)?;
        Engine::new(source, destination, options)
    })();
    match result {
        Ok(engine) => Created {
            handle: Box::into_raw(Box::new(engine)) as u64,
            code: 0,
            backend: 0,
        },
        Err(code) => Created {
            handle: 0,
            code,
            backend: 0,
        },
    }
}

fn span_valid<T>(pointer: *const T, size: i64) -> bool {
    size >= 0 && size <= isize::MAX as i64 && (size == 0 || !pointer.is_null())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_process(
    handle: u64,
    input: *const u8,
    input_size: i64,
    output: *mut u8,
    output_size: i64,
    end: i32,
) -> Step {
    if handle == 0
        || !span_valid(input, input_size)
        || !span_valid(output, output_size)
        || !(0..=1).contains(&end)
    {
        return Step {
            code: 9,
            ..Step::default()
        };
    }
    // SAFETY: The caller owns a live handle, serializes calls, and supplies
    // disjoint readable/writable spans; no managed pointer is retained.
    let engine = unsafe { &mut *(handle as *mut Engine) };
    let input = if input_size == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(input, input_size as usize) }
    };
    let output = if output_size == 0 {
        &mut []
    } else {
        unsafe { slice::from_raw_parts_mut(output, output_size as usize) }
    };
    engine.process(input, output, end == 1)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_free(handle: u64) {
    if handle != 0 {
        // SAFETY: The caller transfers its live handle exactly once after use.
        unsafe {
            drop(Box::from_raw(handle as *mut Engine));
        }
    }
}
