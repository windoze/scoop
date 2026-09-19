use super::*;

fn variant(
    owner: &SourceDeclarationKey,
    name: &str,
) -> CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey> {
    CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(owner, CanonicalIdentifier::new(name).unwrap()).unwrap(),
    )
    .unwrap()
}
fn payload(
    variant: PersistentEnumVariantId,
) -> CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey> {
    CborIdentityRecord::from_key(EnumVariantFieldKey::new(
        variant,
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))
    .unwrap()
}

#[test]
fn enum_replay_selects_pointer_niche_but_keeps_zst_payload_tagged() {
    let owner = source("Optional", SourceNominalKind::Enum, 0);
    let empty = variant(&owner, "Empty");
    let some = variant(&owner, "Some");
    let field = payload(some.id());
    for value in [unit(), managed()] {
        let fields = [EnumLayoutFieldInputV1 {
            field: &field,
            value: &value,
        }];
        let variants = [
            EnumLayoutVariantInputV1 {
                variant: &empty,
                fields: &[],
            },
            EnumLayoutVariantInputV1 {
                variant: &some,
                fields: &fields,
            },
        ];
        let bound = Bound::value(exact(&owner));
        let result = ExactValueLayoutV1::enumeration(
            bound.identity,
            &variants,
            &bound.foundation,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(result.value().storage().byte_size(), 8);
        match result.representation().kind() {
            ExactRepresentationKindV1::TaggedEnum(layout) => {
                assert_eq!(value.value().storage().byte_size(), 0);
                assert_eq!(layout.geometry().tag_layout().byte_size(), 8);
                assert_eq!(layout.variants()[1].fields()[0].storage().offset().get(), 0);
                assert_eq!(encode(result.representation()).unwrap()[2], 5);
            }
            ExactRepresentationKindV1::NicheEnum(layout) => {
                assert_eq!(layout.pointer_kind(), NichePointerKind::Managed);
                assert_eq!(layout.payload_variant(), some.id());
                assert_eq!(scan(&result), &RefScan::References(vec![0]));
                assert_eq!(encode(result.representation()).unwrap()[2], 6);
            }
            _ => panic!("enum representation"),
        }
    }
}

#[test]
fn tagged_enum_combines_dedicated_slots_at_enum_relative_offsets() {
    let owner = source("Mixed", SourceNominalKind::Enum, 0);
    let first = variant(&owner, "Integer");
    let second = variant(&owner, "Reference");
    let fields = [payload(first.id()), payload(second.id())];
    let int = integer("Long", IntegerKind::SIGNED_64);
    let reference = managed();
    let first_fields = [EnumLayoutFieldInputV1 {
        field: &fields[0],
        value: &int,
    }];
    let second_fields = [EnumLayoutFieldInputV1 {
        field: &fields[1],
        value: &reference,
    }];
    let variants = [
        EnumLayoutVariantInputV1 {
            variant: &first,
            fields: &first_fields,
        },
        EnumLayoutVariantInputV1 {
            variant: &second,
            fields: &second_fields,
        },
    ];
    let bound = Bound::value(exact(&owner));
    let value =
        ExactValueLayoutV1::enumeration(bound.identity, &variants, &bound.foundation, &mut meter())
            .unwrap();
    assert_eq!(value.value().storage().byte_size(), 24);
    assert_eq!(scan(&value), &RefScan::References(vec![16]));
    let ExactRepresentationKindV1::TaggedEnum(layout) = value.representation().kind() else {
        panic!("tagged");
    };
    assert_eq!(
        layout
            .variants()
            .iter()
            .map(|variant| variant.fields()[0].storage().offset().get())
            .collect::<Vec<_>>(),
        [8, 16]
    );
}

#[test]
fn enum_replay_rejects_wrong_variant_and_payload_identity_owners() {
    let owner = source("Owner", SourceNominalKind::Enum, 0);
    let first = variant(&owner, "First");
    let other = variant(&source("Other", SourceNominalKind::Enum, 0), "Other");
    let bound = Bound::value(exact(&owner));
    assert!(matches!(
        ExactValueLayoutV1::enumeration(
            bound.identity.clone(),
            &[EnumLayoutVariantInputV1 {
                variant: &other,
                fields: &[]
            }],
            &bound.foundation,
            &mut meter()
        ),
        Err(ExactLayoutReplayError::VariantOwner)
    ));
    let wrong_field = payload(other.id());
    let unit = unit();
    let fields = [EnumLayoutFieldInputV1 {
        field: &wrong_field,
        value: &unit,
    }];
    assert!(matches!(
        ExactValueLayoutV1::enumeration(
            bound.identity.clone(),
            &[EnumLayoutVariantInputV1 {
                variant: &first,
                fields: &fields
            }],
            &bound.foundation,
            &mut meter()
        ),
        Err(ExactLayoutReplayError::VariantFieldOwner)
    ));
    assert!(matches!(
        ExactValueLayoutV1::enumeration(bound.identity, &[], &bound.foundation, &mut meter()),
        Err(ExactLayoutReplayError::EmptyEnum)
    ));
}

#[test]
fn generated_variant_owner_is_verified_without_source_name_fallback() {
    let value = managed();
    let generated = GeneratedNominalKey::CoroutineSlot {
        value: value.identity().exact(),
    };
    let owner = PersistentTypeId::from_generated_key(&generated).unwrap();
    let empty = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::generated(&generated, GeneratedEnumVariantRole::CoroutineSlotEmpty)
            .unwrap(),
    )
    .unwrap();
    let some = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::generated(&generated, GeneratedEnumVariantRole::CoroutineSlotValue)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(empty.key().generated_owner(), Some(owner));
    assert_eq!(empty.key().source_owner(), None);
    let field = payload(some.id());
    let fields = [EnumLayoutFieldInputV1 {
        field: &field,
        value: &value,
    }];
    let variants = [
        EnumLayoutVariantInputV1 {
            variant: &empty,
            fields: &[],
        },
        EnumLayoutVariantInputV1 {
            variant: &some,
            fields: &fields,
        },
    ];
    let bound = Bound::value(CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap());
    let result =
        ExactValueLayoutV1::enumeration(bound.identity, &variants, &bound.foundation, &mut meter())
            .unwrap();
    assert!(matches!(
        result.representation().kind(),
        ExactRepresentationKindV1::NicheEnum(_)
    ));
}
