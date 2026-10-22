use icu_normalizer::ComposingNormalizerBorrowed;

use crate::bytes::{self, TextResult};

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_charset_normalize_nfc(data: *const u8, size: i64) -> TextResult {
    // SAFETY: The C caller keeps the input span readable during this call.
    TextResult::from_text(
        unsafe { bytes::input(data, size) }
            .map(|text| {
                ComposingNormalizerBorrowed::new_nfc()
                    .normalize(text)
                    .into_owned()
            })
            .map_err(|_| 9),
    )
}
