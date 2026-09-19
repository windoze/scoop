//! Untrusted type scan registration carriers.

use super::*;

#[derive(Debug)]
pub(super) enum DecodedTypeDescriptorITableDirectoryV1 {
    Null,
    Defined(DecodedPersistentId<ObjectDefinitionAtomId>),
}

impl WireEncode for DecodedTypeDescriptorITableDirectoryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Null => {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 0, 1)?;
                encode_unsigned_field(encoder, 1, 0)
            }
            Self::Defined(atom) => encode_value_sum(encoder, 2, atom),
        }
    }
}

impl WireDecode for DecodedTypeDescriptorITableDirectoryV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                let marker = decoder.field(1, Decoder::unsigned)?;
                if marker == 0 {
                    Ok(Self::Null)
                } else {
                    Err(unknown_tag(decoder, marker))
                }
            }
            2 => Ok(Self::Defined(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Debug)]
pub(in crate::production::registration_production) enum DecodedTypeDescriptorInlineScanV1 {
    Null,
    Defined(DecodedPersistentId<PersistentScanId>),
}

impl WireEncode for DecodedTypeDescriptorInlineScanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Null => {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 0, 1)?;
                encode_unsigned_field(encoder, 1, 0)
            }
            Self::Defined(scan) => encode_value_sum(encoder, 2, scan),
        }
    }
}

impl WireDecode for DecodedTypeDescriptorInlineScanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                let marker = decoder.field(1, Decoder::unsigned)?;
                if marker == 0 {
                    Ok(Self::Null)
                } else {
                    Err(unknown_tag(decoder, marker))
                }
            }
            2 => Ok(Self::Defined(
                decoder.field(1, DecodedPersistentId::decode)?,
            )),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}
