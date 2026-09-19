//! General layout scan wire. The legacy registration carrier retains its
//! own frozen implementation; this carrier never constructs checked scans.

use std::num::NonZeroU64;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CheckedRefScanV1, NonEmptyRefScan, RefScan, RefScanValidationError};

mod decode;
mod validation;
pub use validation::MeteredScanValidationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedRefScanV1(RawScan);

#[derive(Clone, Debug, Eq, PartialEq)]
enum RawScan {
    None,
    References(Vec<u64>),
    Sequence(Vec<DecodedRefScanV1>),
    Array {
        length_offset: u64,
        first_element_offset: u64,
        stride: u64,
        element: Box<DecodedRefScanV1>,
    },
}

impl DecodedRefScanV1 {
    pub fn validate(self) -> Result<CheckedRefScanV1, RefScanValidationError> {
        CheckedRefScanV1::from_canonical(self.into_scan()?)
    }

    fn into_scan(self) -> Result<RefScan, RefScanValidationError> {
        Ok(match self.0 {
            RawScan::None => RefScan::None,
            RawScan::References(offsets) => RefScan::References(offsets),
            RawScan::Sequence(parts) => RefScan::Sequence(
                parts
                    .into_iter()
                    .map(Self::into_scan)
                    .collect::<Result<_, _>>()?,
            ),
            RawScan::Array {
                length_offset,
                first_element_offset,
                stride,
                element,
            } => {
                let stride =
                    NonZeroU64::new(stride).ok_or(RefScanValidationError::ZeroArrayStride)?;
                let element = NonEmptyRefScan::new(element.into_scan()?)
                    .ok_or(RefScanValidationError::EmptyArrayElement)?;
                RefScan::Array {
                    length_offset,
                    first_element_offset,
                    stride,
                    element: Box::new(element),
                }
            }
        })
    }
}

impl WireDecode for DecodedRefScanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decode::decode(decoder)
    }
}

impl WireEncode for DecodedRefScanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            RawScan::None => tag(encoder, 1, 1),
            RawScan::References(offsets) => encode_references(encoder, offsets),
            RawScan::Sequence(parts) => {
                tag(encoder, 2, 3)?;
                encoder.field(1)?;
                encoder.array(parts.len() as u64)?;
                for part in parts {
                    part.encode(encoder)?;
                }
                Ok(())
            }
            RawScan::Array {
                length_offset,
                first_element_offset,
                stride,
                element,
            } => {
                encode_array_prefix(encoder, *length_offset, *first_element_offset, *stride)?;
                element.encode(encoder)
            }
        }
    }
}

impl WireEncode for CheckedRefScanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_scan(self.as_ref_scan(), encoder)
    }
}

pub(crate) fn encode_scan(
    scan: &RefScan,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match scan {
        RefScan::None => tag(encoder, 1, 1),
        RefScan::References(offsets) => encode_references(encoder, offsets),
        RefScan::Sequence(parts) => {
            tag(encoder, 2, 3)?;
            encoder.field(1)?;
            encoder.array(parts.len() as u64)?;
            for part in parts {
                encode_scan(part, encoder)?;
            }
            Ok(())
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            encode_array_prefix(encoder, *length_offset, *first_element_offset, stride.get())?;
            encode_scan(element.as_ref_scan(), encoder)
        }
    }
}

fn tag(
    encoder: &mut Encoder,
    fields: u64,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(fields)?;
    encoder.field(0)?;
    encoder.unsigned(value)
}

fn encode_references(
    encoder: &mut Encoder,
    offsets: &[u64],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    tag(encoder, 2, 2)?;
    encoder.field(1)?;
    encoder.array(offsets.len() as u64)?;
    for offset in offsets {
        encoder.unsigned(*offset)?;
    }
    Ok(())
}

fn encode_array_prefix(
    encoder: &mut Encoder,
    length: u64,
    first: u64,
    stride: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    tag(encoder, 5, 4)?;
    for (field, value) in [(1, length), (2, first), (3, stride)] {
        encoder.field(field)?;
        encoder.unsigned(value)?;
    }
    encoder.field(4)
}

fn require_length(decoder: &Decoder<'_, '_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
