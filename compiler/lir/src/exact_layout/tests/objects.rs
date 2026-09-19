use super::*;

fn backing(
    owner: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentTypeId, GeneratedNominalKey> {
    CborIdentityRecord::from_key(GeneratedNominalKey::ObjectBackingClass {
        object: PersistentTypeId::from_source_declaration(owner).unwrap(),
    })
    .unwrap()
}

fn property(
    backing: &GeneratedNominalKey,
    name: &str,
) -> CborIdentityRecord<PersistentFieldId, FieldIdentityKey> {
    CborIdentityRecord::from_key(
        FieldIdentityKey::object_backing_property(backing, property_id(name)).unwrap(),
    )
    .unwrap()
}

fn property_id(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

#[test]
fn object_record_keeps_source_exact_and_replays_backing_fields_after_base_prefix() {
    let base_owner = source("Base", SourceNominalKind::Class, 0);
    let base_field = CborIdentityRecord::from_key(
        FieldIdentityKey::source_property_backing(&base_owner, property_id("byte")).unwrap(),
    )
    .unwrap();
    let byte = integer("Byte", IntegerKind::SIGNED_8);
    let bound = Bound::instance(exact(&base_owner));
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
    let object = source("Registry", SourceNominalKind::Object, 0);
    let object_exact = exact(&object);
    let backing = backing(&object);
    let reference = property(backing.key(), "reference");
    let marker = property(backing.key(), "marker");
    let reference_value = managed();
    let unit = unit();
    let bound = Bound::instance(object_exact.clone());
    let layout = ExactInstanceLayoutV1::object(
        bound.identity,
        &backing,
        ClassLayoutBaseV1::Base(&base),
        &[
            NominalLayoutFieldInputV1 {
                field: &reference,
                value: &reference_value,
            },
            NominalLayoutFieldInputV1 {
                field: &marker,
                value: &unit,
            },
        ],
        &bound.foundation,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(layout.identity().exact(), object_exact.id());
    assert_ne!(object_exact.key(), &ExactTypeKey::Nominal(backing.id()));
    assert_eq!(layout.shape().minimum_size(), 32);
    assert_eq!(layout.shape().object_scan(), &RefScan::References(vec![24]));
    let InstanceRepresentationKindV1::ClassObject(class) = layout.representation().kind() else {
        panic!("class object");
    };
    assert_eq!(class.complete_fields().len(), 3);
    assert_eq!(class.declared_fields()[0].storage().offset().get(), 24);
    assert_eq!(class.declared_fields()[1].storage().offset().get(), 0);
    assert_wire_roundtrip(layout);
}

#[test]
fn object_record_requires_matching_backing_key_and_generated_field_owner() {
    let object = source("Registry", SourceNominalKind::Object, 0);
    let correct = backing(&object);
    let other = backing(&source("Other", SourceNominalKind::Object, 0));
    let unit = unit();
    let member = property(correct.key(), "marker");
    let wrong_member = property(other.key(), "marker");
    let bound = Bound::instance(exact(&object));
    assert!(matches!(
        ExactInstanceLayoutV1::class(
            bound.identity.clone(),
            ClassLayoutBaseV1::NoBase,
            &[NominalLayoutFieldInputV1 {
                field: &member,
                value: &unit
            }],
            &bound.foundation,
            &mut meter(),
        ),
        Err(ExactLayoutReplayError::FieldOwner)
    ));
    assert!(matches!(
        ExactInstanceLayoutV1::object(
            bound.identity.clone(),
            &other,
            ClassLayoutBaseV1::NoBase,
            &[],
            &bound.foundation,
            &mut meter(),
        ),
        Err(ExactLayoutReplayError::ObjectBackingIdentity)
    ));
    assert!(matches!(
        ExactInstanceLayoutV1::object(
            bound.identity,
            &correct,
            ClassLayoutBaseV1::NoBase,
            &[NominalLayoutFieldInputV1 {
                field: &wrong_member,
                value: &unit
            }],
            &bound.foundation,
            &mut meter(),
        ),
        Err(ExactLayoutReplayError::FieldOwner)
    ));
}

#[test]
fn object_record_rejects_wrong_generated_family_and_fixed_unit_identity() {
    let unit = unit();
    let wrong = CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue {
        payload: unit.identity().exact(),
    })
    .unwrap();
    let bound = Bound::instance(exact(&source("Registry", SourceNominalKind::Object, 0)));
    assert!(matches!(
        ExactInstanceLayoutV1::object(
            bound.identity,
            &wrong,
            ClassLayoutBaseV1::NoBase,
            &[],
            &bound.foundation,
            &mut meter(),
        ),
        Err(ExactLayoutReplayError::ObjectBackingIdentity)
    ));
    let declaration = CoreBuiltinNominal::Unit.declaration_key();
    let forged_backing = backing(&declaration);
    let bound = Bound::instance(exact(&declaration));
    assert!(matches!(
        ExactInstanceLayoutV1::object(
            bound.identity,
            &forged_backing,
            ClassLayoutBaseV1::NoBase,
            &[],
            &bound.foundation,
            &mut meter(),
        ),
        Err(ExactLayoutReplayError::IdentityKind)
    ));
}
