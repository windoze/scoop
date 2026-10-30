//! Storage-only interning of fixed-width byte strings in already encoded CBOR.
//!
//! Canonical identity/hash encoders remain unchanged. Opaque byte strings are
//! never recursively interpreted as CBOR.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use super::EncodeError;

pub struct Byte32Index {
    indices: BTreeMap<[u8; 32], u64>,
    table: Vec<u8>,
}

impl Byte32Index {
    pub fn from_payloads<'a>(
        payloads: impl IntoIterator<Item = &'a [u8]>,
    ) -> Result<Self, EncodeError> {
        let mut values = BTreeSet::new();
        for payload in payloads {
            visit(payload, |_, value| {
                values.insert(*value);
                Ok(())
            })?;
        }
        let mut table = Vec::new();
        table
            .try_reserve_exact(
                values
                    .len()
                    .checked_mul(32)
                    .ok_or(EncodeError::LengthLimit)?,
            )
            .map_err(|_| EncodeError::Allocation)?;
        let indices = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                table.extend_from_slice(&value);
                (value, index as u64)
            })
            .collect();
        Ok(Self { indices, table })
    }

    pub fn table(&self) -> &[u8] {
        &self.table
    }

    pub fn encode_payload(&self, payload: &[u8]) -> Result<Vec<u8>, EncodeError> {
        let mut output = minicbor::Encoder::new(Vec::new());
        let mut copied = 0;
        visit(payload, |range, value| {
            let index = self.indices.get(value).ok_or_else(|| {
                EncodeError::InvalidValue("byte string is absent from its storage table".into())
            })?;
            output
                .writer_mut()
                .extend_from_slice(&payload[copied..range.start]);
            output
                .u64(*index)
                .map_err(|error| EncodeError::InvalidValue(error.to_string()))?;
            copied = range.end;
            Ok(())
        })?;
        output.writer_mut().extend_from_slice(&payload[copied..]);
        Ok(output.into_writer())
    }
}

fn visit(
    payload: &[u8],
    mut value: impl FnMut(Range<usize>, &[u8; 32]) -> Result<(), EncodeError>,
) -> Result<(), EncodeError> {
    let mut decoder = minicbor::Decoder::new(payload);
    let error = |error: minicbor::decode::Error| EncodeError::InvalidValue(error.to_string());
    while decoder.position() < payload.len() {
        let start = decoder.position();
        // Containers contribute only their head here; the following iterations
        // visit their fields without recursion or an additional value tree.
        match payload[start] >> 5 {
            0 => {
                decoder.u64().map_err(error)?;
            }
            2 => {
                let bytes = decoder.bytes().map_err(error)?;
                if let Ok(bytes) = <&[u8; 32]>::try_from(bytes) {
                    value(start..decoder.position(), bytes)?;
                }
            }
            3 => {
                decoder.str().map_err(error)?;
            }
            4 | 5 => {
                let length = if payload[start] >> 5 == 4 {
                    decoder.array().map_err(error)?
                } else {
                    decoder.map().map_err(error)?
                };
                if length.is_none() {
                    return Err(EncodeError::InvalidValue(
                        "indefinite CBOR container".into(),
                    ));
                }
            }
            _ => {
                return Err(EncodeError::InvalidValue(
                    "value outside Wire CBOR subset".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Decoder;

    #[test]
    fn nested_storage_indices_preserve_integers_and_opaque_byte_strings() {
        let first = [2u8; 32];
        let second = [1u8; 32];
        let mut canonical = minicbor::Encoder::new(Vec::new());
        canonical.map(2).unwrap();
        canonical.u64(1).unwrap();
        canonical.array(3).unwrap();
        canonical.bytes(&first).unwrap();
        canonical.bytes(&second).unwrap();
        canonical.bytes(&first).unwrap();
        canonical.u64(2).unwrap();
        // An opaque blob that itself happens to contain a CBOR byte string.
        let opaque = [&[0x58, 32][..], &first].concat();
        canonical.bytes(&opaque).unwrap();
        let canonical = canonical.into_writer();
        let index = Byte32Index::from_payloads([canonical.as_slice()]).unwrap();
        assert_eq!(index.table(), [second, first].concat());
        let compact = index.encode_payload(&canonical).unwrap();
        assert!(compact.len() + index.table().len() < canonical.len());
        let mut decoder = Decoder::with_byte32_table(&compact, index.table()).unwrap();
        decoder.expect_map(2).unwrap();
        decoder
            .field(1, |decoder| {
                assert_eq!(decoder.array()?, 3);
                assert_eq!(decoder.bytes()?, first);
                assert_eq!(decoder.bytes()?, second);
                assert_eq!(decoder.bytes()?, first);
                Ok(())
            })
            .unwrap();
        assert_eq!(decoder.field(2, Decoder::bytes).unwrap(), opaque);
        decoder.finish().unwrap();
    }
}
