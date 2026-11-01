use crate::bytes::{self, Failure, IdnaResult};
use alloc::string::String;

#[unsafe(no_mangle)]
unsafe extern "C" fn scoop_idna_punycode(data: *const u8, size: i64, decode: bool) -> IdnaResult {
    // SAFETY: The C caller keeps the input span readable during this call.
    let result = unsafe { bytes::input(data, size) }.and_then(|text| {
        if text.is_empty() {
            return Ok(String::new());
        }
        let converted = if decode {
            idna::punycode::decode_to_string(text)
        } else {
            idna::punycode::encode_str(text)
        };
        converted.ok_or_else(|| Failure::new(bytes::PUNYCODE))
    });
    IdnaResult::from_text(result)
}
