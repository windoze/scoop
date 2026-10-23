use icu_casemap::CaseMapper;
use icu_locale_core::LanguageIdentifier;
use icu_properties::{CodePointSetData, props::Alnum};
use icu_segmenter::GraphemeClusterSegmenter;
use unicode_width::UnicodeWidthChar;

use crate::bytes::{self, TextResult};

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_grapheme_breaks(
    data: *const u8,
    size: i64,
    breaks: *mut u8,
) -> i32 {
    // SAFETY: The C caller provides a readable UTF-8 span for this call.
    let Ok(text) = (unsafe { bytes::input(data, size) }) else {
        return 9;
    };
    if text.is_empty() {
        return 0;
    }
    if breaks.is_null() {
        return 9;
    }
    // SAFETY: The caller provides a disjoint writable span of size bytes.
    let output = unsafe { std::slice::from_raw_parts_mut(breaks, text.len()) };
    output.fill(0);
    for offset in GraphemeClusterSegmenter::new().segment_str(text) {
        if offset < output.len() {
            output[offset] = 1;
        }
    }
    0
}

#[unsafe(no_mangle)]
extern "C" fn scoop_charset_scalar_width(scalar: u32) -> i32 {
    if scalar == 0 {
        return -1;
    }
    char::from_u32(scalar)
        .and_then(UnicodeWidthChar::width)
        .map_or(-1, |width| width as i32)
}

#[unsafe(no_mangle)]
extern "C" fn scoop_charset_alphanumeric(scalar: u32) -> bool {
    char::from_u32(scalar)
        .is_some_and(|character| CodePointSetData::new::<Alnum>().contains(character))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_case(data: *const u8, size: i64, mode: i32) -> TextResult {
    let result = (|| {
        // SAFETY: The C caller keeps the input readable during this call.
        let text = unsafe { bytes::input(data, size) }.map_err(|_| 9)?;
        let mapper = CaseMapper::new();
        let locale = LanguageIdentifier::UNKNOWN;
        Ok(match mode {
            0 => mapper.lowercase_to_string(text, &locale),
            1 => mapper.uppercase_to_string(text, &locale),
            2 => mapper.fold_string(text),
            _ => return Err(9),
        }
        .into_owned())
    })();
    TextResult::from_text(result)
}
