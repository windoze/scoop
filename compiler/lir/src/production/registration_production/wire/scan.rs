//! Untrusted scan registration carriers.

use super::*;

#[derive(Debug)]
pub(in crate::production::registration_production) enum DecodedRefScan {
    None,
    References(Vec<u64>),
    Sequence(Vec<Self>),
    Array {
        length_offset: u64,
        first_element_offset: u64,
        stride: u64,
        element: Box<Self>,
    },
}

impl WireEncode for DecodedRefScan {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => encode_empty_sum(encoder, 1),
            Self::References(offsets) => {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 0, 2)?;
                encoder.field(1)?;
                encoder.array(offsets.len() as u64)?;
                for offset in offsets {
                    encoder.unsigned(*offset)?;
                }
                Ok(())
            }
            Self::Sequence(parts) => {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 0, 3)?;
                encode_array_field(encoder, 1, parts)
            }
            Self::Array {
                length_offset,
                first_element_offset,
                stride,
                element,
            } => {
                encoder.map(5)?;
                encode_unsigned_field(encoder, 0, 4)?;
                encode_unsigned_field(encoder, 1, *length_offset)?;
                encode_unsigned_field(encoder, 2, *first_element_offset)?;
                encode_unsigned_field(encoder, 3, *stride)?;
                encode_field(encoder, 4, element.as_ref())
            }
        }
    }
}

impl WireDecode for DecodedRefScan {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let length = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, length, 1)?;
                Ok(Self::None)
            }
            2 => {
                require_sum_length(decoder, length, 2)?;
                let offsets = decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| decoder.unsigned())
                })?;
                Ok(Self::References(offsets))
            }
            3 => {
                require_sum_length(decoder, length, 2)?;
                let parts = decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| Self::decode(decoder))
                })?;
                Ok(Self::Sequence(parts))
            }
            4 => {
                require_sum_length(decoder, length, 5)?;
                Ok(Self::Array {
                    length_offset: decoder.field(1, Decoder::unsigned)?,
                    first_element_offset: decoder.field(2, Decoder::unsigned)?,
                    stride: decoder.field(3, Decoder::unsigned)?,
                    element: decoder.field(4, |decoder| Self::decode(decoder).map(Box::new))?,
                })
            }
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl DecodedRefScan {
    pub(in crate::production::registration_production) fn read(
        self,
    ) -> Result<crate::RefScan, crate::LinkDataError> {
        use crate::{NonEmptyRefScan, RefScan, link_data::link_error};
        Ok(match self {
            Self::None => RefScan::None,
            Self::References(offsets) => RefScan::References(offsets),
            Self::Sequence(parts) => RefScan::Sequence(
                parts
                    .into_iter()
                    .map(Self::read)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Array {
                length_offset,
                first_element_offset,
                stride,
                element,
            } => RefScan::Array {
                length_offset,
                first_element_offset,
                stride: std::num::NonZeroU64::new(stride)
                    .ok_or_else(|| link_error("zero array scan stride"))?,
                element: Box::new(
                    NonEmptyRefScan::new(element.read()?)
                        .ok_or_else(|| link_error("empty array element scan"))?,
                ),
            },
        })
    }
}
