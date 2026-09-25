use scoop_identity::*;
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::exact_layout::tests::{Bound, exact, field, integer, managed, source, unit};
use crate::*;

mod definitions;
mod fixtures;
mod instances;
mod projection;
mod values;

fn decode(expected: &ExactLayoutExportV1) -> DecodedExactLayoutExportV1 {
    decode_canonical(&encode(expected).unwrap()).unwrap()
}

fn roundtrip(expected: &ExactLayoutExportV1) {
    let decoded = decode(expected);
    assert_eq!(encode(&decoded).unwrap(), encode(expected).unwrap());
    assert_eq!(decoded.validate_against(expected).unwrap(), *expected);
}

fn reject(expected: &ExactLayoutExportV1, edit: impl FnOnce(&mut DecodedExactLayoutExportV1)) {
    let mut raw = decode(expected);
    edit(&mut raw);
    // Mutations still form canonical CBOR. Semantic refinement must reject them.
    let raw = decode_canonical::<DecodedExactLayoutExportV1>(&encode(&raw).unwrap()).unwrap();
    assert!(raw.validate_against(expected).is_err());
}

fn value(raw: &mut DecodedExactLayoutExportV1) -> &mut RawValue {
    match &mut raw.semantic.body {
        RawBody::Value { representation, .. } => representation,
        _ => panic!("value"),
    }
}

fn instance(raw: &mut DecodedExactLayoutExportV1) -> &mut RawInstance {
    match &mut raw.semantic.body {
        RawBody::Instance { representation, .. } => representation,
        _ => panic!("instance"),
    }
}

#[test]
fn complete_record_checks_every_header_and_physical_definition() {
    let expected = ExactLayoutExportV1::from(unit());
    let other = ExactLayoutExportV1::from(integer("Byte", IntegerKind::SIGNED_8));
    roundtrip(&expected);
    reject(&expected, |raw| {
        raw.semantic.layout = decode(&other).semantic.layout
    });
    reject(&expected, |raw| {
        raw.semantic.exact = decode(&other).semantic.exact
    });
    reject(&expected, |raw| {
        raw.semantic.scan = decode(&other).semantic.scan
    });
    reject(&expected, |raw| raw.definition = decode(&other).definition);
    reject(&expected, |raw| {
        raw.semantic.role = RepresentationRole::CValue
    });
    reject(&expected, |raw| {
        raw.semantic.target = decode_canonical(
            &encode(&CapabilityId::new("test", "foreign-target", 1).unwrap()).unwrap(),
        )
        .unwrap()
    });
    let bytes = fixtures::bytes();
    reject(&expected, |raw| {
        raw.semantic.body = decode(&bytes).semantic.body
    });
}

#[test]
fn complete_record_decoder_rejects_noncanonical_map_and_unknown_closed_tags() {
    let expected = ExactLayoutExportV1::from(unit());
    let mut bytes = encode(&expected).unwrap();
    bytes[0] = 0xa6;
    assert!(decode_canonical::<DecodedExactLayoutExportV1>(&bytes).is_err());
    for (bytes, valid) in [
        (vec![0xa2, 0, 7, 1, 0xa1, 0, 1], true),
        (vec![0xa2, 0, 7, 1, 0xa1, 0, 2], false),
        (vec![0xa2, 0, 2, 1, 0xa1, 0, 4], false),
        (vec![0xa2, 0, 8, 1, 0xa1, 0, 1], false),
        (vec![0xa3, 0, 7, 1, 0xa1, 0, 1, 2, 0], false),
    ] {
        assert_eq!(decode_canonical::<RawValue>(&bytes).is_ok(), valid);
    }
}
