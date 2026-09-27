use super::*;

#[test]
fn scalar_pointer_unit_records_have_unique_closed_kinds_and_complete_definitions() {
    for kind in IntegerKind::ALL {
        let value = integer(kind.canonical_name(), kind);
        assert_eq!(value.value().storage().byte_size(), kind.width().bytes());
        assert!(
            matches!(value.representation().kind(), ExactRepresentationKindV1::Scalar(ScalarRepresentationKindV1::Integer(actual)) if actual == kind)
        );
        let record = ExactLayoutExportV1::from(value);
        assert_eq!(
            record.scan_definition().subject(),
            ExternalStrongShapeSubjectV1::Scan(record.scan())
        );
        assert_eq!(encode(&record).unwrap()[0], 0xa7);
        assert_wire_roundtrip(record);
    }
    let value = unit();
    assert_eq!(value.value().storage().byte_size(), 0);
    assert_eq!(
        encode(value.representation()).unwrap(),
        [0xa2, 0, 7, 1, 0xa1, 0, 1]
    );
    let reference = managed();
    assert_wire_roundtrip(reference.clone());
    assert_eq!(reference.value().storage().byte_size(), 8);
    assert_eq!(scan(&reference), &RefScan::References(vec![0]));
    let boolean = Bound::value(exact(&source("Boolean", SourceNominalKind::Struct, 0)));
    let boolean = ExactValueLayoutV1::scalar(
        boolean.identity,
        ScalarRepresentationKindV1::Boolean,
        &boolean.foundation,
    )
    .unwrap();
    assert_eq!(boolean.value().storage().byte_size(), 1);
    assert_wire_roundtrip(boolean.clone());
    assert_eq!(
        encode(boolean.representation()).unwrap(),
        [0xa2, 0, 1, 1, 0xa1, 0, 2]
    );
    for (key, kind) in [
        (
            ExactTypeKey::RawPointer(value.identity().exact()),
            NichePointerKind::Raw,
        ),
        (
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: vec![],
                result: value.identity().exact(),
            },
            NichePointerKind::Code,
        ),
    ] {
        let bound = Bound::value(CborIdentityRecord::from_key(key).unwrap());
        let pointer =
            ExactValueLayoutV1::qualified_pointer(bound.identity, kind, &bound.foundation).unwrap();
        assert_eq!(scan(&pointer), &RefScan::None);
        assert_wire_roundtrip(pointer);
    }
}

#[test]
fn ordinary_struct_and_tuple_replay_typed_dependencies_and_field_order() {
    let owner = source("Fields", SourceNominalKind::Struct, 0);
    let fields = [
        field(&owner, "unit"),
        field(&owner, "byte"),
        field(&owner, "reference"),
    ];
    let unit = unit();
    let byte = integer("Byte", IntegerKind::SIGNED_8);
    let reference = managed();
    let values = [&unit, &byte, &reference];
    let inputs: Vec<_> = fields
        .iter()
        .zip(values)
        .map(|(field, value)| NominalLayoutFieldInputV1 {
            field,
            value: value.value(),
        })
        .collect();
    let bound = Bound::value(exact(&owner));
    let structure =
        ExactValueLayoutV1::ordinary_struct(bound.identity, true, &inputs, &bound.foundation)
            .unwrap();
    assert_eq!(structure.value().storage().byte_size(), 16);
    assert_eq!(scan(&structure), &RefScan::References(vec![8]));
    let ExactRepresentationKindV1::Struct(representation) = structure.representation().kind()
    else {
        panic!("struct");
    };
    assert!(representation.interior_mutable());
    assert_eq!(
        representation
            .fields()
            .iter()
            .map(|field| field.storage().offset().get())
            .collect::<Vec<_>>(),
        [0, 0, 8]
    );
    let tuple = Bound::value(
        CborIdentityRecord::from_key(ExactTypeKey::Tuple(
            NonEmptyVec::new(
                values
                    .iter()
                    .map(|value| value.identity().exact())
                    .collect(),
            )
            .unwrap(),
        ))
        .unwrap(),
    );
    let tuple = ExactValueLayoutV1::tuple(tuple.identity, &values, &tuple.foundation).unwrap();
    assert_eq!(tuple.value().storage(), structure.value().storage());
    assert_eq!(encode(tuple.representation()).unwrap()[2], 4);
    assert_wire_roundtrip(structure);
    assert_wire_roundtrip(tuple);
}

#[test]
fn records_reject_wrong_role_identity_kind_owner_and_missing_scan_relation() {
    let owner = source("Owner", SourceNominalKind::Struct, 0);
    let bound = Bound::instance(exact(&owner));
    assert!(matches!(
        ExactValueLayoutV1::unit(bound.identity, &bound.foundation),
        Err(ExactLayoutReplayError::RepresentationRole)
    ));
    let bound = Bound::value(exact(&owner));
    assert!(matches!(
        ExactValueLayoutV1::qualified_pointer(
            bound.identity.clone(),
            NichePointerKind::Raw,
            &bound.foundation
        ),
        Err(ExactLayoutReplayError::IdentityKind)
    ));
    let foreign = field(&source("Foreign", SourceNominalKind::Struct, 0), "field");
    let value = unit();
    assert!(matches!(
        ExactValueLayoutV1::ordinary_struct(
            bound.identity.clone(),
            false,
            &[NominalLayoutFieldInputV1 {
                field: &foreign,
                value: value.value()
            }],
            &bound.foundation
        ),
        Err(ExactLayoutReplayError::FieldOwner)
    ));
    assert!(matches!(
        ExactValueLayoutV1::unit(bound.identity, &bound.foundation),
        Err(ExactLayoutReplayError::IdentityKind)
    ));
    let bound = Bound::value(exact(&CoreBuiltinNominal::Unit.declaration_key()));
    let mut canonical = bound.foundation.as_canonical().clone();
    canonical.set_scans(Vec::new()).unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    assert!(matches!(
        ExactValueLayoutV1::unit(bound.identity, &foundation),
        Err(ExactLayoutReplayError::MissingScan)
    ));
}

#[test]
fn fixed_unit_identity_cannot_claim_another_representation() {
    let bound = Bound::value(exact(&CoreBuiltinNominal::Unit.declaration_key()));
    for kind in [
        ScalarRepresentationKindV1::Integer(IntegerKind::SIGNED_8),
        ScalarRepresentationKindV1::Boolean,
    ] {
        assert!(matches!(
            ExactValueLayoutV1::scalar(bound.identity.clone(), kind, &bound.foundation),
            Err(ExactLayoutReplayError::IdentityKind)
        ));
    }
    assert!(matches!(
        ExactValueLayoutV1::ordinary_struct(bound.identity.clone(), false, &[], &bound.foundation),
        Err(ExactLayoutReplayError::IdentityKind)
    ));
    assert!(matches!(
        ExactValueLayoutV1::qualified_pointer(
            bound.identity,
            NichePointerKind::Managed,
            &bound.foundation
        ),
        Err(ExactLayoutReplayError::IdentityKind)
    ));
    let bound = Bound::instance(exact(&CoreBuiltinNominal::Unit.declaration_key()));
    assert!(matches!(
        ExactInstanceLayoutV1::inline_bytes(bound.identity.clone(), &bound.foundation),
        Err(ExactLayoutReplayError::IdentityKind)
    ));
    assert!(matches!(
        ExactInstanceLayoutV1::abstract_reference(bound.identity, &bound.foundation),
        Err(ExactLayoutReplayError::IdentityKind)
    ));
}
