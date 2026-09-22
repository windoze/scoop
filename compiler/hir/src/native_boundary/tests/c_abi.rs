use super::*;
use crate::NativeBoundaryCAbiV1 as CAbi;

fn scalar_record(
    fixture: &Fixture,
    c_layout: NativeBoundaryCLayoutPolicy,
) -> NativeBoundaryTypeDefinitionRecord {
    NativeBoundaryTypeDefinitionRecord::new(
        &fixture.structure.1,
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout,
            fields: vec![
                NativeBoundaryFieldDefinition::new(&fixture.fields[0].1, fixture.value_type())
                    .unwrap(),
            ],
        },
    )
    .unwrap()
}

fn with_unchecked_projection(
    record: &NativeBoundaryTypeDefinitionRecord,
    projection: CAbi,
) -> Vec<u8> {
    let mut bytes = encode(record).unwrap();
    assert_eq!(&bytes[bytes.len() - 4..], &[4, 0xa1, 0, 1]);
    bytes.truncate(bytes.len() - 3);
    bytes.extend(encode(&projection).unwrap());
    bytes
}

#[test]
fn explicit_c_projections_roundtrip_complete_typed_members() {
    let fixture = Fixture::new();
    let scalar = CAbi::UInt64Field {
        field: fixture.fields[0].0,
    };
    let nullable = CAbi::NullablePointer {
        none: fixture.variants[0].0,
        payload: fixture.variant_fields[0].0,
    };
    for record in [
        fixture.reference_record(),
        scalar_record(&fixture, NativeBoundaryCLayoutPolicy::NotCLayout)
            .with_c_abi(scalar)
            .unwrap(),
        fixture.enum_record().with_c_abi(nullable).unwrap(),
    ] {
        let bytes = encode(&record).unwrap();
        assert_eq!(bytes[0], 0xa4);
        let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
            &bytes,
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), record);
    }
}

#[test]
fn scalar_projection_requires_the_only_field_of_an_ordinary_struct() {
    let fixture = Fixture::new();
    let projection = CAbi::UInt64Field {
        field: fixture.fields[0].0,
    };
    for record in [
        fixture.reference_record(),
        fixture.struct_record(),
        fixture.generic_struct_record(),
        fixture.enum_record(),
        scalar_record(
            &fixture,
            NativeBoundaryCLayoutPolicy::CLayout {
                aligned: CLayoutOverride::Natural,
                packed: CLayoutOverride::Natural,
            },
        ),
    ] {
        assert_eq!(
            record.clone().with_c_abi(projection),
            Err(NativeBoundaryDefinitionError::CAbiProjectionMismatch)
        );
        let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
            &with_unchecked_projection(&record, projection),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(decoded.resolve(&mut fixture.resolver()).is_err());
    }
    for field in [fixture.fields[1].0, fixture.foreign_field.0] {
        let record = scalar_record(&fixture, NativeBoundaryCLayoutPolicy::NotCLayout);
        let projection = CAbi::UInt64Field { field };
        assert_eq!(
            record.clone().with_c_abi(projection),
            Err(NativeBoundaryDefinitionError::CAbiProjectionMismatch)
        );
        let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
            &with_unchecked_projection(&record, projection),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(decoded.resolve(&mut fixture.resolver()).is_err());
    }
}

#[test]
fn nullable_projection_requires_the_exact_empty_and_single_payload_variants() {
    let fixture = Fixture::new();
    let projection = CAbi::NullablePointer {
        none: fixture.variants[0].0,
        payload: fixture.variant_fields[0].0,
    };
    let NativeBoundaryNominalShape::Enum { variants } = fixture.enum_record().shape().clone()
    else {
        unreachable!()
    };
    for variants in [
        vec![variants[0].clone()],
        vec![
            variants[0].clone(),
            NativeBoundaryVariantDefinition::new(&fixture.variants[1].1, Vec::new()).unwrap(),
        ],
    ] {
        let record = NativeBoundaryTypeDefinitionRecord::new(
            &fixture.enumeration.1,
            &[0],
            NativeBoundaryNominalShape::Enum { variants },
        )
        .unwrap();
        assert_eq!(
            record.clone().with_c_abi(projection),
            Err(NativeBoundaryDefinitionError::CAbiProjectionMismatch)
        );
        let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
            &with_unchecked_projection(&record, projection),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(decoded.resolve(&mut fixture.resolver()).is_err());
    }
    let wrong_none = CAbi::NullablePointer {
        none: fixture.variants[1].0,
        payload: fixture.variant_fields[0].0,
    };
    let record = fixture.enum_record();
    assert_eq!(
        record.clone().with_c_abi(wrong_none),
        Err(NativeBoundaryDefinitionError::CAbiProjectionMismatch)
    );
    let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
        &with_unchecked_projection(&record, wrong_none),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(decoded.resolve(&mut fixture.resolver()).is_err());
}

#[test]
fn native_record_rejects_missing_projection_and_unknown_projection_tags() {
    let fixture = Fixture::new();
    let mut legacy = encode(&fixture.reference_record()).unwrap();
    legacy[0] = 0xa3;
    legacy.truncate(legacy.len() - 4);
    assert!(
        decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
            &legacy,
            DecodeLimits::default()
        )
        .is_err()
    );
    let mut bytes = encode(&fixture.reference_record()).unwrap();
    *bytes.last_mut().unwrap() = 4;
    let error = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.path(), &scoop_wire::WirePath::root().field(4));
}
