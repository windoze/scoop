use super::*;

#[test]
fn semantic_facts_exclude_managed_zst_and_gc_free_references() {
    assert!(matches!(
        MirTypeFactsV1::try_new(
            MirValueKindV1::ZeroSizedValue,
            MirGcKindV1::ContainsManagedReferences
        ),
        Err(MirTypeBridgeError::ContradictoryFacts)
    ));
    assert!(matches!(
        MirTypeFactsV1::try_new(MirValueKindV1::Reference, MirGcKindV1::GcFree),
        Err(MirTypeBridgeError::ContradictoryFacts)
    ));
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.source(
            fixture.empty.id(),
            facts(MirValueKindV1::ZeroSizedValue, MirGcKindV1::GcFree),
            MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Integer(
                crate::IntegerKind::SIGNED_32
            ))
        ),
        Err(MirTypeBridgeError::RepresentationFactsMismatch { .. })
    ));
}

#[test]
fn source_origin_cannot_relabel_another_exact_or_representation_family() {
    let fixture = Fixture::new();
    let empty = fixture.empty_export();
    assert!(matches!(
        ParamFreeMirTypeExportV1::try_new(
            fixture.authority(),
            empty.exact(),
            MirTypeOriginV1::SourceNominal(fixture.other.id()),
            empty.facts(),
            empty.representation().clone(),
            no_bases()
        ),
        Err(MirTypeBridgeError::ExactOriginMismatch { .. })
    ));
    assert!(matches!(
        fixture.source(
            fixture.class.id(),
            empty.facts(),
            empty.representation().clone()
        ),
        Err(MirTypeBridgeError::OriginRepresentationMismatch { .. })
    ));
    let boxed = fixture.boxed_export();
    assert!(matches!(
        ParamFreeMirTypeExportV1::try_new(
            fixture.authority(),
            boxed.exact(),
            MirTypeOriginV1::SourceNominal(boxed.origin().nominal()),
            boxed.facts(),
            boxed.representation().clone(),
            no_bases()
        ),
        Err(MirTypeBridgeError::Reference(_))
    ));
}

#[test]
fn field_order_survives_but_foreign_and_duplicate_fields_are_rejected() {
    let fixture = Fixture::new();
    let fields: Vec<_> = fixture
        .fields
        .iter()
        .map(|field| MirRepresentationFieldV1 {
            field: field.id(),
            value: fixture.payload.id(),
        })
        .collect();
    let shape = MirTypeRepresentationV1::Struct {
        fields: fields.clone(),
        c_layout: MirTypeCLayoutPolicyV1::Ordinary,
        interior_mutable: true,
    };
    let exported = fixture
        .source(
            fixture.empty.id(),
            facts(MirValueKindV1::ZeroSizedValue, MirGcKindV1::GcFree),
            shape.clone(),
        )
        .unwrap();
    assert_eq!(exported.representation().fields(), fields);
    assert!(matches!(
        fixture.source(fixture.other.id(), exported.facts(), shape),
        Err(MirTypeBridgeError::FieldOwner { .. })
    ));
    let duplicate = MirTypeRepresentationV1::Struct {
        fields: vec![fields[0].clone(), fields[0].clone()],
        c_layout: MirTypeCLayoutPolicyV1::Ordinary,
        interior_mutable: false,
    };
    assert!(matches!(
        fixture.source(fixture.empty.id(), exported.facts(), duplicate),
        Err(MirTypeBridgeError::DuplicateField { .. })
    ));
}

#[test]
fn generated_helpers_require_canonical_roles_payloads_and_complete_variant_order() {
    let fixture = Fixture::new();
    let boxed = fixture.boxed_export();
    let bad = MirTypeRepresentationV1::BoxedValue {
        payload: MirRepresentationFieldV1 {
            field: fixture.boxed.payload_field_record().id(),
            value: exact(fixture.other.id()).id(),
        },
    };
    assert!(matches!(
        ParamFreeMirTypeExportV1::try_new(
            fixture.authority(),
            boxed.exact(),
            boxed.origin().clone(),
            boxed.facts(),
            bad,
            no_bases()
        ),
        Err(MirTypeBridgeError::GeneratedPayloadMismatch { .. })
    ));
    let step = fixture.step_export();
    let MirTypeRepresentationV1::CoroutineStep { mut variants } = step.representation().clone()
    else {
        unreachable!()
    };
    variants.reverse();
    assert!(matches!(
        ParamFreeMirTypeExportV1::try_new(
            fixture.authority(),
            step.exact(),
            step.origin().clone(),
            step.facts(),
            MirTypeRepresentationV1::CoroutineStep { variants },
            no_bases()
        ),
        Err(MirTypeBridgeError::GeneratedPayloadMismatch { .. })
    ));
    let missing =
        crate::OdrFreeMirFoundation::try_new(crate::CanonicalMirFoundation::empty()).unwrap();
    assert!(matches!(
        ParamFreeMirTypeExportV1::try_new(
            MirTypeBridgeAuthority {
                identities: &fixture.graph,
                foundation: &missing
            },
            boxed.exact(),
            boxed.origin().clone(),
            boxed.facts(),
            boxed.representation().clone(),
            no_bases()
        ),
        Err(MirTypeBridgeError::MissingGeneratedFoundation { .. })
    ));
}

#[test]
fn enum_facts_use_the_conjunction_of_variant_gc_free_facts() {
    let fixture = Fixture::new();
    let shape = MirTypeRepresentationV1::Enum {
        variants: vec![MirRepresentationVariantV1 {
            variant: fixture.variant.id(),
            fields: vec![MirRepresentationVariantFieldV1 {
                field: fixture.variant_field.id(),
                value: exact(fixture.class.id()).id(),
            }],
            gc: MirGcKindV1::ContainsManagedReferences,
        }],
    };
    fixture
        .source(
            fixture.enumeration.id(),
            facts(
                MirValueKindV1::NonZeroValue,
                MirGcKindV1::ContainsManagedReferences,
            ),
            shape.clone(),
        )
        .unwrap();
    assert!(matches!(
        fixture.source(
            fixture.enumeration.id(),
            facts(MirValueKindV1::NonZeroValue, MirGcKindV1::GcFree),
            shape
        ),
        Err(MirTypeBridgeError::RepresentationFactsMismatch { .. })
    ));
}

#[test]
fn bases_and_interfaces_keep_exact_kind_and_do_not_accept_duplicates() {
    let fixture = Fixture::new();
    let interface = exact(fixture.interface.id()).id();
    let class = fixture
        .source(
            fixture.class.id(),
            facts(
                MirValueKindV1::Reference,
                MirGcKindV1::ContainsManagedReferences,
            ),
            MirTypeRepresentationV1::Class {
                kind: MirClassKindV1::Abstract,
                declared_fields: vec![],
            },
        )
        .unwrap();
    let rebuild = |relation| {
        ParamFreeMirTypeExportV1::try_new(
            fixture.authority(),
            class.exact(),
            class.origin().clone(),
            class.facts(),
            class.representation().clone(),
            relation,
        )
    };
    rebuild(MirBaseAndInterfacesV1 {
        base: MirBaseClassV1::None,
        interfaces: vec![interface],
    })
    .unwrap();
    let mut interfaces = vec![interface, exact(fixture.other_interface.id()).id()];
    interfaces.sort_unstable();
    rebuild(MirBaseAndInterfacesV1 {
        base: MirBaseClassV1::None,
        interfaces: interfaces.clone(),
    })
    .unwrap();
    interfaces.reverse();
    assert!(matches!(
        rebuild(MirBaseAndInterfacesV1 {
            base: MirBaseClassV1::None,
            interfaces
        }),
        Err(MirTypeBridgeError::NonCanonicalInterfaces { index: 1 })
    ));
    assert!(matches!(
        rebuild(MirBaseAndInterfacesV1 {
            base: MirBaseClassV1::None,
            interfaces: vec![interface, interface]
        }),
        Err(MirTypeBridgeError::DuplicateInterface { .. })
    ));
    assert!(matches!(
        rebuild(MirBaseAndInterfacesV1 {
            base: MirBaseClassV1::Base(fixture.payload.id()),
            interfaces: vec![]
        }),
        Err(MirTypeBridgeError::InvalidBase { .. })
    ));
    assert!(matches!(
        rebuild(MirBaseAndInterfacesV1 {
            base: MirBaseClassV1::None,
            interfaces: vec![class.exact()]
        }),
        Err(MirTypeBridgeError::InvalidInterface { .. })
    ));
}
