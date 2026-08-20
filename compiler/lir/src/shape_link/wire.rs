use scoop_wire::{
    Decoder, Encoder, WireEncode, WireError, WireErrorKind, WirePath, encode_canonical_temporary,
};

use super::ShapeLinkError;

pub(super) type EncodeResult = Result<(), scoop_wire::cbor::EncodeError>;
pub(super) fn field(encoder: &mut Encoder, index: u32, value: &impl WireEncode) -> EncodeResult {
    encoder.field(index)?;
    value.encode(encoder)
}
pub(super) fn unknown(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}
pub(super) fn length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        return Ok(());
    }
    Err(WireError::new(
        WireErrorKind::InvalidLength { expected, actual },
        decoder.path().clone(),
        Some(decoder.position()),
    ))
}
pub(super) fn equal_fields(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
) -> Result<bool, ShapeLinkError> {
    let path = WirePath::root();
    let actual = encode_canonical_temporary(actual, &path)?;
    let expected = encode_canonical_temporary(expected, &path)?;

    Ok(actual == expected)
}
