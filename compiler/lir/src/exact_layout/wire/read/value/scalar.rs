use super::*;
use crate::{IntegerKind, IntegerSignedness, IntegerWidth};

pub(super) fn decode(decoder: &mut Decoder<'_>) -> Result<ScalarRepresentationKindV1, WireError> {
    let fields = decoder.map()?;
    match decoder.field(0, Decoder::unsigned)? {
        1 => {
            length(decoder, fields, 3)?;
            let signedness = decoder.field(1, |decoder| match tag(decoder)? {
                1 => Ok(IntegerSignedness::Signed),
                2 => Ok(IntegerSignedness::Unsigned),
                value => Err(unknown(decoder, value)),
            })?;
            let width = decoder.field(2, |decoder| match tag(decoder)? {
                1 => Ok(IntegerWidth::W8),
                2 => Ok(IntegerWidth::W16),
                3 => Ok(IntegerWidth::W32),
                4 => Ok(IntegerWidth::W64),
                value => Err(unknown(decoder, value)),
            })?;
            Ok(ScalarRepresentationKindV1::Integer(IntegerKind::new(
                signedness, width,
            )))
        }
        2 => {
            length(decoder, fields, 1)?;
            Ok(ScalarRepresentationKindV1::Boolean)
        }
        3 => {
            length(decoder, fields, 1)?;
            Ok(ScalarRepresentationKindV1::Char)
        }
        value => Err(unknown(decoder, value)),
    }
}

pub(super) fn alignment(decoder: &mut Decoder<'_>) -> Result<CLayoutOverride, WireError> {
    use scoop_identity::CLayoutByteAlignment as A;
    let alignment = match tag(decoder)? {
        1 => return Ok(CLayoutOverride::Natural),
        2 => A::Bytes1,
        3 => A::Bytes2,
        4 => A::Bytes4,
        5 => A::Bytes8,
        6 => A::Bytes16,
        value => return Err(unknown(decoder, value)),
    };
    Ok(CLayoutOverride::Bytes(alignment))
}

fn tag(decoder: &mut Decoder<'_>) -> Result<u64, WireError> {
    decoder.expect_map(1)?;
    decoder.field(0, Decoder::unsigned)
}
