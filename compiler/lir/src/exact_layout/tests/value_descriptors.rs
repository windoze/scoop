use super::*;

fn own_descriptor(value: &ExactValueLayoutV1) -> ExactInstanceLayoutV1 {
    let bound = Bound::instance(value.identity().exact_record().clone());
    ExactInstanceLayoutV1::boxed_payload(bound.identity, value, &bound.foundation, &mut meter())
        .unwrap()
}

#[test]
fn source_value_descriptors_keep_same_exact_and_boxed_instance_shape() {
    let unit = unit();
    let integer = integer("Int", IntegerKind::SIGNED_64);
    let zero = Bound::value(exact(&source("Empty", SourceNominalKind::Struct, 0)));
    let zero = ExactValueLayoutV1::ordinary_struct(
        zero.identity,
        false,
        &[],
        &zero.foundation,
        &mut meter(),
    )
    .unwrap();
    for (value, minimum, inline_size) in [(&unit, 16, 0), (&zero, 16, 0), (&integer, 24, 8)] {
        let descriptor = own_descriptor(value);
        assert_eq!(descriptor.identity().exact(), value.identity().exact());
        assert_eq!(
            descriptor.shape().instance_kind(),
            TypeInstanceKindV1::BoxedValue
        );
        assert_eq!(descriptor.shape().minimum_size(), minimum);
        assert_eq!(descriptor.shape().inline_size(), inline_size);
        assert_eq!(descriptor.shape().object_scan(), &RefScan::None);
        assert_wire_roundtrip(descriptor);
    }
}

#[test]
fn source_aggregate_descriptor_translates_payload_scan_from_same_exact_layout() {
    let owner = source("WithReference", SourceNominalKind::Struct, 0);
    let field = field(&owner, "value");
    let reference = managed();
    let bound = Bound::value(exact(&owner));
    let value = ExactValueLayoutV1::ordinary_struct(
        bound.identity,
        false,
        &[NominalLayoutFieldInputV1 {
            field: &field,
            value: &reference,
        }],
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    let descriptor = own_descriptor(&value);
    assert_eq!(
        descriptor.shape().inline_scan(),
        &RefScan::References(vec![0])
    );
    assert_eq!(
        descriptor.shape().object_scan(),
        &RefScan::References(vec![16])
    );
    assert_eq!(descriptor.shape().minimum_size(), 24);
    assert_wire_roundtrip(descriptor);
}

#[test]
fn boxed_payload_rejects_reference_value_and_c_storage_role() {
    let value = managed();
    let own = Bound::instance(value.identity().exact_record().clone());
    assert!(matches!(
        ExactInstanceLayoutV1::boxed_payload(own.identity, &value, &own.foundation, &mut meter()),
        Err(ExactLayoutReplayError::BoxPayloadKind)
    ));
    let integer = exact(&source("CInteger", SourceNominalKind::Struct, 0));
    let bound = Bound::new(
        integer.clone(),
        RepresentationRole::CValue,
        ScanRole::InlineValue,
    );
    let value = ExactValueLayoutV1::scalar(
        bound.identity,
        ScalarRepresentationKindV1::Integer(IntegerKind::SIGNED_64),
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    let own = Bound::instance(integer);
    assert!(matches!(
        ExactInstanceLayoutV1::boxed_payload(own.identity, &value, &own.foundation, &mut meter()),
        Err(ExactLayoutReplayError::RepresentationRole)
    ));
}
