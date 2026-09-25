//! Mutate schema-valid fields inside an already verified fixture payload.

use super::*;
use std::ops::Range;

pub(super) fn capability(failure: Failure) -> scoop_identity::CapabilityId {
    match failure {
        Failure::DefinedMissing
        | Failure::DefinedDuplicate
        | Failure::DefinedMember
        | Failure::LegacyMissing
        | Failure::LegacyWidth => slib::lir_link_identity_closure_capability(),
        Failure::OrdinaryMissing | Failure::OrdinaryDuplicate | Failure::OrdinaryIndex => {
            slib::lir_cross_cone_link_closure_capability()
        }
        Failure::ShapeMissing | Failure::ShapeDuplicate | Failure::ShapeIndex => {
            slib::lir_cross_cone_layout_link_closure_capability()
        }
        Failure::UnknownRuntime | Failure::WrongNativeSymbol => panic!("physical symbol mutation"),
    }
}

pub(super) fn mutate(bytes: &[u8], failure: Failure) -> Vec<u8> {
    let field = match failure {
        Failure::DefinedMissing | Failure::DefinedDuplicate | Failure::DefinedMember => 4,
        Failure::LegacyMissing | Failure::LegacyWidth => 5,
        _ => 2,
    };
    let range = field_range(bytes, field);
    let records = array_parts(&bytes[range.clone()]);
    assert!(!records.is_empty());
    let mut owned = records
        .iter()
        .map(|record| record.to_vec())
        .collect::<Vec<_>>();
    match failure {
        Failure::DefinedMissing
        | Failure::LegacyMissing
        | Failure::OrdinaryMissing
        | Failure::ShapeMissing => {
            owned.remove(0);
        }
        Failure::DefinedDuplicate | Failure::OrdinaryDuplicate | Failure::ShapeDuplicate => {
            owned.insert(0, owned[0].clone());
        }
        Failure::DefinedMember => {
            let member = field_range(&owned[0], 1);
            owned[0][member.end - 1] ^= 1;
        }
        Failure::LegacyWidth => {
            let use_ = field_range(&owned[0], 1);
            let changed = replace_field(&owned[0][use_.clone()], 6, &encode(&Unsigned(1)).unwrap());
            owned[0].splice(use_, changed);
        }
        Failure::OrdinaryIndex | Failure::ShapeIndex => {
            owned[0] = replace_field(&owned[0], 2, &encode(&Unsigned(u32::MAX.into())).unwrap());
        }
        Failure::UnknownRuntime | Failure::WrongNativeSymbol => panic!("physical symbol mutation"),
    }
    let mut changed = bytes.to_vec();
    changed.splice(range, encode_array_records(&owned));
    changed
}

pub(super) fn encode_array_records(records: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = encode(&ArrayHeader(records.len() as u64)).unwrap();
    for record in records {
        bytes.extend_from_slice(record);
    }
    bytes
}

pub(super) fn replace_field(bytes: &[u8], field: u64, replacement: &[u8]) -> Vec<u8> {
    let mut changed = bytes.to_vec();
    changed.splice(field_range(bytes, field), replacement.iter().copied());
    changed
}

pub(super) fn field_range(bytes: &[u8], field: u64) -> Range<usize> {
    let mut budget = meter();
    let mut decoder = scoop_wire::Decoder::new(bytes, &mut budget).unwrap();
    let count = decoder.map().unwrap();
    let mut found = None;
    for _ in 0..count {
        let key = decoder.unsigned().unwrap();
        let start = decoder.position() as usize;
        skip(bytes, &mut decoder);
        if key == field {
            found = Some(start..decoder.position() as usize);
        }
    }
    decoder.finish().unwrap();
    found.unwrap()
}

pub(super) fn array_parts(bytes: &[u8]) -> Vec<&[u8]> {
    let mut budget = meter();
    let mut decoder = scoop_wire::Decoder::new(bytes, &mut budget).unwrap();
    let count = decoder.array().unwrap();
    let mut parts = Vec::new();
    for _ in 0..count {
        let start = decoder.position() as usize;
        skip(bytes, &mut decoder);
        parts.push(&bytes[start..decoder.position() as usize]);
    }
    decoder.finish().unwrap();
    parts
}

fn skip(bytes: &[u8], decoder: &mut scoop_wire::Decoder<'_, '_>) {
    match bytes[decoder.position() as usize] >> 5 {
        0 => {
            decoder.unsigned().unwrap();
        }
        2 => {
            decoder.bytes().unwrap();
        }
        3 => {
            decoder.text().unwrap();
        }
        4 => {
            for _ in 0..decoder.array().unwrap() {
                skip(bytes, decoder);
            }
        }
        5 => {
            for _ in 0..decoder.map().unwrap() {
                decoder.unsigned().unwrap();
                skip(bytes, decoder);
            }
        }
        tag => panic!("unexpected canonical fixture item {tag}"),
    }
}

struct Unsigned(u64);
impl WireEncode for Unsigned {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.0)
    }
}

struct ArrayHeader(u64);
impl WireEncode for ArrayHeader {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0)
    }
}
