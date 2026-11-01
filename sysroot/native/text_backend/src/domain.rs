use alloc::{borrow::Cow, string::String};
use core::ptr;

use idna::uts46::{AsciiDenyList, ErrorPolicy, Hyphens, ProcessingError, ProcessingSuccess, Uts46};

use crate::bytes::{self, Failure, IdnaResult};

struct Name<'a> {
    ascii: Cow<'a, str>,
    unicode: Cow<'a, str>,
}

fn convert(input: &str, label_only: bool) -> Result<Name<'_>, Failure> {
    if input.is_empty() {
        return Err(Failure::new(bytes::EMPTY));
    }
    // The root domain is an existing API boundary, separate from label validity.
    if matches!(input, "." | "\u{3002}" | "\u{ff0e}" | "\u{ff61}") {
        return if label_only {
            Err(Failure::at(bytes::LABEL, 0))
        } else {
            Ok(Name {
                ascii: Cow::Borrowed("."),
                unicode: Cow::Borrowed("."),
            })
        };
    }
    let ascii_input = input.is_ascii();
    let mut label = 0;
    for c in input.chars() {
        if matches!(c, '.' | '\u{3002}' | '\u{ff0e}' | '\u{ff61}') {
            label += 1;
        } else if c.is_ascii_control() || (ascii_input && !(c.is_ascii_alphanumeric() || c == '-'))
        {
            return Err(Failure::at(bytes::DISALLOWED, label));
        }
    }
    let mut unicode = String::new();
    let mut ascii = String::new();
    let name = match Uts46::new().process(
        input.as_bytes(),
        AsciiDenyList::STD3,
        Hyphens::Check,
        ErrorPolicy::FailFast,
        |_, _, _| true,
        &mut unicode,
        Some(&mut ascii),
    ) {
        Ok(ProcessingSuccess::Passthrough) => Name {
            ascii: Cow::Borrowed(input),
            unicode: Cow::Borrowed(input),
        },
        Ok(ProcessingSuccess::WroteToSink) => Name {
            ascii: Cow::Owned(if ascii.is_empty() {
                unicode.clone()
            } else {
                ascii
            }),
            unicode: Cow::Owned(unicode),
        },
        Err(ProcessingError::ValidityError) => return Err(Failure::new(bytes::INVALID)),
        Err(ProcessingError::SinkError) => return Err(Failure::new(bytes::BACKEND)),
    };
    check_dns(&name.ascii, label_only)?;
    Ok(name)
}

fn check_dns(ascii: &str, label_only: bool) -> Result<(), Failure> {
    if label_only && ascii.contains('.') {
        return Err(Failure::new(bytes::LABEL));
    }
    let without_root = ascii.strip_suffix('.').unwrap_or(ascii);
    for (index, label) in without_root.split('.').enumerate() {
        if label.is_empty() {
            return Err(Failure::at(bytes::LABEL, index));
        }
        if label.len() > 63 {
            return Err(Failure::at(bytes::LENGTH, index));
        }
    }
    // Each textual dot takes the space of a label length octet; add the first
    // length octet and terminal root octet to obtain DNS wire length.
    if without_root.len() > 253 {
        return Err(Failure::new(bytes::LENGTH));
    }
    Ok(())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_idna_domain(data: *const u8, size: i64, mode: i32) -> IdnaResult {
    let result = (|| {
        if !(0..=3).contains(&mode) {
            return Err(Failure::new(bytes::ARGUMENT));
        }
        // SAFETY: The C caller keeps the input span readable during this call.
        let text = unsafe { bytes::input(data, size) }?;
        let name = convert(text, mode >= 2)?;
        Ok(if mode & 1 == 0 {
            name.ascii
        } else {
            name.unicode
        }
        .into_owned())
    })();
    IdnaResult::from_text(result)
}

#[repr(C)]
struct IdnaName {
    ascii: *mut u8,
    unicode: *mut u8,
    ascii_size: i64,
    unicode_size: i64,
    code: i32,
    label: i64,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_idna_parse(data: *const u8, size: i64) -> IdnaName {
    // SAFETY: The C caller keeps the input span readable during this call.
    match unsafe { bytes::input(data, size) }.and_then(|text| convert(text, false)) {
        Ok(name) => {
            let (ascii, ascii_size) = bytes::owned(name.ascii.into_owned());
            let (unicode, unicode_size) = bytes::owned(name.unicode.into_owned());
            IdnaName {
                ascii,
                unicode,
                ascii_size,
                unicode_size,
                code: 0,
                label: -1,
            }
        }
        Err(error) => IdnaName {
            ascii: ptr::null_mut(),
            unicode: ptr::null_mut(),
            ascii_size: 0,
            unicode_size: 0,
            code: error.code,
            label: error.label,
        },
    }
}
