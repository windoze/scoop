use scoop_wire::{Decoder, Encoder, WireEncode, WireError, WireErrorKind};

pub(super) fn tag(
    encoder: &mut Encoder,
    fields: u64,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(fields)?;
    encoder.field(0)?;
    encoder.unsigned(value)
}

pub(super) fn expect_fields(
    decoder: &Decoder<'_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

pub(super) fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

pub(super) fn sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}
