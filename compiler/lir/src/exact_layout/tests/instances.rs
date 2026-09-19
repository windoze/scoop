use super::*;

fn backing(
    owner: &SourceDeclarationKey,
    name: &str,
) -> CborIdentityRecord<PersistentFieldId, FieldIdentityKey> {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            owner.origin(),
            owner.package().clone(),
            owner.owners().clone(),
            owner.scope().clone(),
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap();
    CborIdentityRecord::from_key(
        FieldIdentityKey::source_property_backing(owner, property).unwrap(),
    )
    .unwrap()
}

#[test]
fn class_record_preserves_complete_base_prefix_and_identity_bound_fields() {
    let base = source("Base", SourceNominalKind::Class, 0);
    let derived = source("Derived", SourceNominalKind::Class, 0);
    let base_field = backing(&base, "base");
    let derived_field = backing(&derived, "derived");
    let byte = integer("Byte", IntegerKind::SIGNED_8);
    let bound = Bound::instance(exact(&base));
    let base = ExactInstanceLayoutV1::class(
        bound.identity,
        ClassLayoutBaseV1::NoBase,
        &[NominalLayoutFieldInputV1 {
            field: &base_field,
            value: &byte,
        }],
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(base.shape().minimum_size(), 24);
    let bound = Bound::instance(exact(&derived));
    let derived = ExactInstanceLayoutV1::class(
        bound.identity,
        ClassLayoutBaseV1::Base(&base),
        &[NominalLayoutFieldInputV1 {
            field: &derived_field,
            value: &byte,
        }],
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    let InstanceRepresentationKindV1::ClassObject(layout) = derived.representation().kind() else {
        panic!("class");
    };
    assert_eq!(layout.declared_fields()[0].storage().offset().get(), 24);
    assert_eq!(layout.complete_fields().len(), 2);
    assert_eq!(derived.shape().minimum_size(), 32);
    assert_eq!(
        encode(&ExactLayoutExportV1::from(derived)).unwrap()[0],
        0xa7
    );
}

#[test]
fn box_array_bytes_and_abstract_records_rebuild_all_shape_fields() {
    let unit = unit();
    let boxed = PersistentTypeId::from_generated_key(&GeneratedNominalKey::BoxedValue {
        payload: unit.identity().exact(),
    })
    .unwrap();
    let bound =
        Bound::instance(CborIdentityRecord::from_key(ExactTypeKey::Nominal(boxed)).unwrap());
    let boxed = ExactInstanceLayoutV1::boxed_payload(
        bound.identity,
        &unit,
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(boxed.shape().minimum_size(), 16);
    assert_eq!(
        boxed.shape().inline_storage_kind(),
        InlineStorageKindV1::ZeroSized
    );
    let element = managed();
    let array = source("Array", SourceNominalKind::Struct, 1);
    let array = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
        origin: PersistentGenericTypeId::from_source_declaration(&array).unwrap(),
        arguments: NonEmptyVec::from_first(element.identity().exact(), []),
    })
    .unwrap();
    let bound = Bound::new(
        array,
        RepresentationRole::ManagedObject,
        ScanRole::ArrayElement,
    );
    let array = ExactInstanceLayoutV1::inline_array(
        bound.identity,
        &element,
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(array.shape().inline_stride(), 8);
    assert!(matches!(
        array.shape().object_scan(),
        RefScan::Array {
            first_element_offset: 24,
            ..
        }
    ));
    let record = ExactLayoutExportV1::from(array);
    assert_eq!(
        record.scan(),
        PersistentScanId::from_key(&ScanKey::new(
            record.identity().layout(),
            ScanRole::ArrayElement
        ))
        .unwrap()
    );
    let bound = Bound::instance(exact(&source("String", SourceNominalKind::Class, 0)));
    let bytes =
        ExactInstanceLayoutV1::inline_bytes(bound.identity, &bound.foundation, &mut meter())
            .unwrap();
    assert_eq!(bytes.shape().inline_stride(), 1);
    let bound = Bound::instance(exact(&source("Interface", SourceNominalKind::Interface, 0)));
    let abstract_ref =
        ExactInstanceLayoutV1::abstract_reference(bound.identity, &bound.foundation, &mut meter())
            .unwrap();
    assert_eq!(abstract_ref.shape(), &TypeInstanceShapeV1::abstract_ref());
}

#[test]
fn instance_replay_rejects_box_identity_array_scan_role_and_bounded_prefix_copy() {
    let owner = source("Owner", SourceNominalKind::Class, 0);
    let bound = Bound::instance(exact(&owner));
    let unit = unit();
    assert!(matches!(
        ExactInstanceLayoutV1::boxed_payload(
            bound.identity.clone(),
            &unit,
            &bound.foundation,
            &mut meter()
        ),
        Err(ExactLayoutReplayError::BoxPayloadIdentity)
    ));
    let base = ExactInstanceLayoutV1::class(
        bound.identity,
        ClassLayoutBaseV1::NoBase,
        &[],
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    let derived = Bound::instance(exact(&source("Derived", SourceNominalKind::Class, 0)));
    let limits = DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        ExactInstanceLayoutV1::class(
            derived.identity,
            ClassLayoutBaseV1::Base(&base),
            &[],
            &derived.foundation,
            &mut BudgetMeter::new(limits)
        ),
        Err(ExactLayoutReplayError::Resource(_))
    ));
}
