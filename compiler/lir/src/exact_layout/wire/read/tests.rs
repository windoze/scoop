use scoop_identity::*;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::exact_layout::tests::{Bound, exact, field, integer, managed, meter, source, unit};
use crate::*;

mod definitions;
mod fixtures;
mod instances;
mod values;

fn decode(expected: &ExactLayoutExportV1) -> DecodedExactLayoutExportV1 {
    decode_canonical(&encode(expected).unwrap(), DecodeLimits::default()).unwrap()
}

fn roundtrip(expected: &ExactLayoutExportV1) {
    let decoded = decode(expected);
    assert_eq!(encode(&decoded).unwrap(), encode(expected).unwrap());
    assert_eq!(
        decoded.validate_against(expected, &mut meter()).unwrap(),
        *expected
    );
}

fn reject(expected: &ExactLayoutExportV1, edit: impl FnOnce(&mut DecodedExactLayoutExportV1)) {
    let mut raw = decode(expected);
    edit(&mut raw);
    // Mutations still form canonical CBOR. Semantic refinement must reject them.
    let raw = decode_canonical::<DecodedExactLayoutExportV1>(
        &encode(&raw).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(raw.validate_against(expected, &mut meter()).is_err());
}

fn value(raw: &mut DecodedExactLayoutExportV1) -> &mut RawValue {
    match &mut raw.body {
        RawBody::Value { representation, .. } => representation,
        _ => panic!("value"),
    }
}

fn instance(raw: &mut DecodedExactLayoutExportV1) -> &mut RawInstance {
    match &mut raw.body {
        RawBody::Instance { representation, .. } => representation,
        _ => panic!("instance"),
    }
}

#[test]
fn complete_record_checks_every_header_and_physical_definition() {
    let expected = ExactLayoutExportV1::from(unit());
    let other = ExactLayoutExportV1::from(integer("Byte", IntegerKind::SIGNED_8));
    roundtrip(&expected);
    reject(&expected, |raw| raw.layout = decode(&other).layout);
    reject(&expected, |raw| raw.exact = decode(&other).exact);
    reject(&expected, |raw| raw.scan = decode(&other).scan);
    reject(&expected, |raw| raw.definition = decode(&other).definition);
    reject(&expected, |raw| raw.role = RepresentationRole::CValue);
    reject(&expected, |raw| {
        raw.target = decode_canonical(
            &encode(&CapabilityId::new("test", "foreign-target", 1).unwrap()).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap()
    });
    let bytes = fixtures::bytes();
    reject(&expected, |raw| raw.body = decode(&bytes).body);
}

#[test]
fn complete_record_bounds_work_before_promoting_checked_data() {
    let expected = fixtures::aggregate();
    let limits = DecodeLimits {
        validation_work_units: 1,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        decode(&expected).validate_against(&expected, &mut BudgetMeter::new(limits)),
        Err(ExactLayoutWireError::Resource(_))
    ));
    let limits = DecodeLimits {
        owned_bytes: 1,
        ..DecodeLimits::default()
    };
    assert!(
        decode(&expected)
            .validate_against(&expected, &mut BudgetMeter::new(limits))
            .is_err()
    );
    let limits = DecodeLimits {
        semantic_table_entries: 1,
        ..DecodeLimits::default()
    };
    assert!(
        decode_canonical::<DecodedExactLayoutExportV1>(&encode(&expected).unwrap(), limits)
            .is_err()
    );
}

#[test]
fn complete_record_decoder_rejects_noncanonical_map_and_unknown_closed_tags() {
    let expected = ExactLayoutExportV1::from(unit());
    let mut bytes = encode(&expected).unwrap();
    bytes[0] = 0xa6;
    assert!(
        decode_canonical::<DecodedExactLayoutExportV1>(&bytes, DecodeLimits::default()).is_err()
    );
    for (bytes, valid) in [
        (vec![0xa2, 0, 7, 1, 0xa1, 0, 1], true),
        (vec![0xa2, 0, 7, 1, 0xa1, 0, 2], false),
        (vec![0xa2, 0, 2, 1, 0xa1, 0, 4], false),
        (vec![0xa2, 0, 8, 1, 0xa1, 0, 1], false),
        (vec![0xa3, 0, 7, 1, 0xa1, 0, 1, 2, 0], false),
    ] {
        assert_eq!(
            decode_canonical::<RawValue>(&bytes, DecodeLimits::default()).is_ok(),
            valid
        );
    }
}
