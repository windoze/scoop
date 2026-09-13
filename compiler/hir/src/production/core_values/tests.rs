use scoop_identity::{
    AccessorRole, BindingTarget, CanonicalIdentifier, CborIdentityRecord, DeclarationName,
    DeclarationScope, DefinitionOrigin, DefinitionOriginRecord, DefinitionOwnerChain,
    ExportBindingKey, NonEmptyVec, NormalizedSourcePath, PackagePath, PersistentPropertyAccessorId,
    PropertyAccessorKey, PropertyOwner as IdentityPropertyOwner, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn value_surface_has_fixed_wire_for_every_definition_and_capability_variant() {
    let fixture = fixture();
    let bytes = encode(&fixture.surface).unwrap();
    assert_eq!(
        hex(&bytes),
        "84a4015820192a52a67cc4f027adf0f0db5b15f357e07d97b220544c22ced3dc9177412a4902a200020158203d365168fed1d5def4845755760513dd2b0cf8801309d92c9c25ccc4ab5eb84003a2000201a301a1000102a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b503a20001015820f3b8bb97e8b1e178a16cf16bba9f9e9fde4ba3c13bb663ab888984eb726582e804a2000101a3000201a10001025820ba3a2277b6c2dc18604fd5b344ae9d9403abcbe3d55829f2a0d07cddd2503b36a40158201e066eb496796bf19f29edbc1c3c26dd90ecae8bfd0ab002a7b3cfb51161dc3202a2000201582024d35ca4cb3302cde1125c58a416720aa9819d7e99fd93b036a8f5c8a258a17303a2000201a301a1000102a200030182a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b5a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b503a30002015820608f4039ba9140edb48c78e29fbc27d0a61e0ae2e78da220bbbb24608eb912b20258203608b8f11638e825dec55e2e0ac6ae3ca995874d42b65f2a4eb7ae73a3aae0f804a2000201a3000201a10001025820e903528dd5072ab4d40edeae0b51a27fd456c72411201ec669867b52539545b7a4015820cd4e0a2414d29257e8c9c79fd6b9752b8b6bf66f003fa87c936af482ddee0f6202a200010158209beec0b6471a20d1eb0b8bf42ea8f4c2cc3b4742f3e0650561805d4f66fee98903a200010158201a9910131cbbe86846e72f2ab740cb5936f64d68fd844d09d4ac2147e970090e04a2000101a200010158205f4aff7128b85b9bfa7f0526494b27c5876e3e7ab98c3253f1a02f44ee622c6da4015820ed46eca599f4d53522edcde6988cd592864aeaa30a07233e2910f4718b6b834c02a20003015820324672f10b84210165ca7325822faab677ba9dcb9dbe491868781482b83b29ff03a2000201a301a2000201a300070100020002a300070100020003a200010158204a7664192ef771251520cf984566961f2f3fff3150aef9012bc97f95439cfdb404a200030101"
    );

    assert_eq!(
        decode(&fixture.surface).validate_against(&fixture.foundation, &fixture.direct),
        Ok(fixture.surface)
    );
}

#[test]
fn value_surface_reader_rejects_unknown_missing_and_extra_fields() {
    for bytes in [
        vec![
            0x81, 0xa3, 0x01, 0x40, 0x02, 0xa1, 0x00, 0x01, 0x03, 0xa1, 0x00, 0x01,
        ],
        vec![
            0x81, 0xa5, 0x01, 0x40, 0x02, 0xa1, 0x00, 0x01, 0x03, 0xa1, 0x00, 0x01, 0x04, 0xa1,
            0x00, 0x01, 0x05, 0x00,
        ],
    ] {
        assert!(
            decode_canonical::<DecodedCoreValueTargetSurfaceV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa1, 0x00, 0x04],
        vec![0xa1, 0x00, 0x01],
        vec![0xa3, 0x00, 0x01, 0x01, 0x40, 0x02, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreValueDefinitionV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
        assert!(
            decode_canonical::<DecodedCoreHirValueCapabilityV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa1, 0x00, 0x03],
        vec![0xa1, 0x00, 0x01],
        vec![0xa3, 0x00, 0x01, 0x01, 0x40, 0x02, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreValueSourceInterfaceV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
        assert!(
            decode_canonical::<DecodedCoreExactValueInterfaceV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa1, 0x00, 0x03],
        vec![0xa1, 0x00, 0x01],
        vec![0xa4, 0x00, 0x02, 0x01, 0x40, 0x02, 0x40, 0x03, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCorePropertyAccessorsV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

#[test]
fn value_surface_requires_canonical_total_direct_value_coverage() {
    let fixture = fixture();
    let targets = fixture.surface.targets().to_vec();

    let missing = raw_surface(&targets[..targets.len() - 1]);
    assert_eq!(
        missing.validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::Coverage {
            expected: targets.len(),
            actual: targets.len() - 1,
        })
    );

    let mut reversed = targets.clone();
    reversed.reverse();
    assert!(matches!(
        raw_surface(&reversed).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::NonCanonicalOrder { .. })
    ));

    let mut duplicate = targets.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(matches!(
        raw_surface(&duplicate).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::DuplicateBinding { .. })
    ));

    let unknown = property("unknown");
    let unknown_binding = value_binding(unknown.key()).id();
    let unknown_direct = CanonicalDirectPublicSurfaceV1::try_new(vec![unknown_binding]).unwrap();
    assert!(matches!(
        decode(&fixture.surface).validate_against(&fixture.foundation, &unknown_direct),
        Err(CoreValueTargetSurfaceValidationError::UnknownDirectSurfaceBinding(binding))
            if binding == unknown_binding
    ));
}

#[test]
fn object_value_requires_its_source_type_binding_origin_and_exact_type() {
    let fixture = fixture();
    let object_index = target_index(&fixture, |definition| {
        matches!(definition, CoreValueDefinitionV1::ObjectValue(_))
    });

    let direct_without_type = CanonicalDirectPublicSurfaceV1::try_new(
        fixture
            .direct
            .bindings()
            .iter()
            .copied()
            .filter(|binding| *binding != fixture.object_type_binding)
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        decode(&fixture.surface).validate_against(&fixture.foundation, &direct_without_type),
        Err(CoreValueTargetSurfaceValidationError::MissingObjectTypeBinding { .. })
    ));

    let mut wrong_source_type = fixture.surface.targets().to_vec();
    wrong_source_type[object_index].source_interface = CoreValueSourceInterfaceV1::ObjectValue {
        source_type: fixture.text_type,
    };
    assert!(matches!(
        raw_surface(&wrong_source_type).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::ObjectTypeMismatch(_))
    ));

    let mut wrong_exact = fixture.surface.targets().to_vec();
    wrong_exact[object_index].capability = CoreHirValueCapabilityV1::ParamFreeStrong(
        CoreExactValueInterfaceV1::ObjectValue(fixture.text_exact),
    );
    assert!(matches!(
        raw_surface(&wrong_exact).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::ExactInterfaceMismatch(_))
    ));

    let mut wrong_branch = fixture.surface.targets().to_vec();
    wrong_branch[object_index].capability = CoreHirValueCapabilityV1::StructuralUnavailable(
        CoreExactValueInterfaceV1::ObjectValue(fixture.object_exact),
    );
    assert!(matches!(
        raw_surface(&wrong_branch).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(_))
    ));

    let mut without_origin = fixture.foundation.clone();
    without_origin
        .set_definition_origins(
            fixture
                .origins
                .iter()
                .filter(|record| {
                    record.subject() != DefinitionOriginSubject::Type(fixture.object_type)
                })
                .cloned()
                .collect(),
        )
        .unwrap();
    assert!(matches!(
        decode(&fixture.surface).validate_against(&without_origin, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::MissingDefinitionOrigin(
            DefinitionOriginSubject::Type(id)
        )) if id == fixture.object_type
    ));
}

#[test]
fn property_value_replays_receiver_accessors_exact_types_and_capability() {
    let fixture = fixture();
    let strong_index = target_index(&fixture, |definition| {
        definition == CoreValueDefinitionV1::Property(fixture.strong_property)
    });
    let structural_index = target_index(&fixture, |definition| {
        definition == CoreValueDefinitionV1::Property(fixture.structural_property)
    });
    let generic_index = target_index(&fixture, |definition| {
        matches!(definition, CoreValueDefinitionV1::ExtensionProperty(_))
    });

    let mut wrong_definition = fixture.surface.targets().to_vec();
    wrong_definition[strong_index].definition = wrong_definition[generic_index].definition;
    assert!(matches!(
        raw_surface(&wrong_definition).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::DefinitionMismatch(_))
    ));

    let mut wrong_receiver = fixture.surface.targets().to_vec();
    let CoreValueSourceInterfaceV1::Property(interface) =
        &mut wrong_receiver[generic_index].source_interface
    else {
        panic!("fixture generic target must be a property")
    };
    interface.receiver = OptionalSignatureType::Absent;
    assert!(matches!(
        raw_surface(&wrong_receiver).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::ReceiverMismatch(_))
    ));

    let mut invalid_binder = fixture.surface.targets().to_vec();
    let CoreValueSourceInterfaceV1::Property(interface) =
        &mut invalid_binder[generic_index].source_interface
    else {
        panic!("fixture generic target must be a property")
    };
    interface.value = SignatureTypeKey::Binder { depth: 1, index: 0 };
    assert!(matches!(
        raw_surface(&invalid_binder).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::InvalidBinder { .. })
    ));

    let mut wrong_accessor = fixture.surface.targets().to_vec();
    let CoreValueSourceInterfaceV1::Property(interface) =
        &mut wrong_accessor[strong_index].source_interface
    else {
        panic!("fixture strong target must be a property")
    };
    interface.accessors = CorePropertyAccessorsV1::ReadOnly {
        getter: fixture.structural_getter,
    };
    assert!(matches!(
        raw_surface(&wrong_accessor).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::AccessorOwnerMismatch)
    ));

    let mut wrong_exact = fixture.surface.targets().to_vec();
    wrong_exact[strong_index].capability =
        CoreHirValueCapabilityV1::ParamFreeStrong(CoreExactValueInterfaceV1::Property {
            receiver: OptionalExactOwner::Absent,
            value: fixture.tuple_exact,
        });
    assert!(matches!(
        raw_surface(&wrong_exact).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::ExactInterfaceMismatch(_))
    ));

    let mut wrong_structural_branch = fixture.surface.targets().to_vec();
    wrong_structural_branch[structural_index].capability =
        CoreHirValueCapabilityV1::ParamFreeStrong(CoreExactValueInterfaceV1::Property {
            receiver: OptionalExactOwner::Absent,
            value: fixture.tuple_exact,
        });
    assert!(matches!(
        raw_surface(&wrong_structural_branch)
            .validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(_))
    ));

    let mut wrong_generic_count = fixture.surface.targets().to_vec();
    wrong_generic_count[generic_index].capability = CoreHirValueCapabilityV1::GenericUnavailable {
        type_parameter_count: 2,
    };
    assert!(matches!(
        raw_surface(&wrong_generic_count).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(_))
    ));
}

fn decode(surface: &CoreValueTargetSurfaceV1) -> DecodedCoreValueTargetSurfaceV1 {
    decode_canonical(&encode(surface).unwrap(), DecodeLimits::default()).unwrap()
}

fn raw_surface(targets: &[CoreValueTargetV1]) -> DecodedCoreValueTargetSurfaceV1 {
    struct RawSurface<'a>(&'a [CoreValueTargetV1]);

    impl WireEncode for RawSurface<'_> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array(self.0.len() as u64)?;
            for target in self.0 {
                target.encode(encoder)?;
            }
            Ok(())
        }
    }

    decode_canonical(
        &encode(&RawSurface(targets)).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}

fn target_index(fixture: &Fixture, predicate: impl Fn(CoreValueDefinitionV1) -> bool) -> usize {
    fixture
        .surface
        .targets()
        .iter()
        .position(|target| predicate(target.definition()))
        .unwrap()
}

struct Fixture {
    foundation: CanonicalHirFoundation,
    direct: CanonicalDirectPublicSurfaceV1,
    surface: CoreValueTargetSurfaceV1,
    origins: Vec<DefinitionOriginRecord>,
    object_type: PersistentTypeId,
    text_type: PersistentTypeId,
    strong_property: PersistentPropertyId,
    structural_property: PersistentPropertyId,
    structural_getter: PersistentPropertyAccessorId,
    object_type_binding: PersistentExportBindingId,
    object_exact: PersistentExactTypeId,
    text_exact: PersistentExactTypeId,
    tuple_exact: PersistentExactTypeId,
}

fn fixture() -> Fixture {
    let object_source = nominal("Console", SourceNominalKind::Object, 0);
    let object_type = PersistentTypeId::from_source_declaration(&object_source).unwrap();
    let object_value =
        CborIdentityRecord::<PersistentObjectValueId, _>::from_key(object_source.clone()).unwrap();
    let text_source = nominal("Text", SourceNominalKind::Class, 0);
    let text = CborIdentityRecord::<PersistentTypeId, _>::from_key(text_source).unwrap();
    let text_type = text.id();

    let object_exact_record = exact(ExactTypeKey::Nominal(object_type));
    let text_exact_record = exact(ExactTypeKey::Nominal(text_type));
    let tuple_exact_record = exact(ExactTypeKey::Tuple(
        NonEmptyVec::new(vec![text_exact_record.id(), text_exact_record.id()]).unwrap(),
    ));
    let object_exact = object_exact_record.id();
    let text_exact = text_exact_record.id();
    let tuple_exact = tuple_exact_record.id();

    let strong_property = property("version");
    let structural_property = property("pair");
    let generic_property = extension_property("identity");

    let strong_getter = accessor(
        IdentityPropertyOwner::Property(strong_property.id()),
        AccessorRole::Getter,
    );
    let structural_getter = accessor(
        IdentityPropertyOwner::Property(structural_property.id()),
        AccessorRole::Getter,
    );
    let structural_setter = accessor(
        IdentityPropertyOwner::Property(structural_property.id()),
        AccessorRole::Setter,
    );
    let generic_getter = accessor(
        IdentityPropertyOwner::ExtensionProperty(generic_property.id()),
        AccessorRole::Getter,
    );

    let object_type_binding = type_binding(&object_source);
    let object_value_binding = value_binding(object_value.key());
    let strong_binding = value_binding(strong_property.key());
    let structural_binding = value_binding(structural_property.key());
    let generic_binding = value_binding(generic_property.key());

    let surface = CoreValueTargetSurfaceV1::try_new(vec![
        CoreValueTargetV1 {
            binding: generic_binding.id(),
            definition: CoreValueDefinitionV1::ExtensionProperty(generic_property.id()),
            source_interface: CoreValueSourceInterfaceV1::Property(CorePropertySourceInterfaceV1 {
                receiver: OptionalSignatureType::Present(Box::new(SignatureTypeKey::Binder {
                    depth: 0,
                    index: 0,
                })),
                value: SignatureTypeKey::Binder { depth: 0, index: 0 },
                accessors: CorePropertyAccessorsV1::ReadOnly {
                    getter: generic_getter.id(),
                },
            }),
            capability: CoreHirValueCapabilityV1::GenericUnavailable {
                type_parameter_count: 1,
            },
        },
        CoreValueTargetV1 {
            binding: structural_binding.id(),
            definition: CoreValueDefinitionV1::Property(structural_property.id()),
            source_interface: CoreValueSourceInterfaceV1::Property(CorePropertySourceInterfaceV1 {
                receiver: OptionalSignatureType::Absent,
                value: SignatureTypeKey::Tuple(
                    NonEmptyVec::new(vec![
                        SignatureTypeKey::Nominal(text_type),
                        SignatureTypeKey::Nominal(text_type),
                    ])
                    .unwrap(),
                ),
                accessors: CorePropertyAccessorsV1::ReadWrite {
                    getter: structural_getter.id(),
                    setter: structural_setter.id(),
                },
            }),
            capability: CoreHirValueCapabilityV1::StructuralUnavailable(
                CoreExactValueInterfaceV1::Property {
                    receiver: OptionalExactOwner::Absent,
                    value: tuple_exact,
                },
            ),
        },
        CoreValueTargetV1 {
            binding: strong_binding.id(),
            definition: CoreValueDefinitionV1::Property(strong_property.id()),
            source_interface: CoreValueSourceInterfaceV1::Property(CorePropertySourceInterfaceV1 {
                receiver: OptionalSignatureType::Absent,
                value: SignatureTypeKey::Nominal(text_type),
                accessors: CorePropertyAccessorsV1::ReadOnly {
                    getter: strong_getter.id(),
                },
            }),
            capability: CoreHirValueCapabilityV1::ParamFreeStrong(
                CoreExactValueInterfaceV1::Property {
                    receiver: OptionalExactOwner::Absent,
                    value: text_exact,
                },
            ),
        },
        CoreValueTargetV1 {
            binding: object_value_binding.id(),
            definition: CoreValueDefinitionV1::ObjectValue(object_value.id()),
            source_interface: CoreValueSourceInterfaceV1::ObjectValue {
                source_type: object_type,
            },
            capability: CoreHirValueCapabilityV1::ParamFreeStrong(
                CoreExactValueInterfaceV1::ObjectValue(object_exact),
            ),
        },
    ])
    .unwrap();

    let origins = vec![
        origin_record(DefinitionOriginSubject::Type(object_type)),
        origin_record(DefinitionOriginSubject::Property(strong_property.id())),
        origin_record(DefinitionOriginSubject::Property(structural_property.id())),
        origin_record(DefinitionOriginSubject::ExtensionProperty(
            generic_property.id(),
        )),
        origin_record(DefinitionOriginSubject::PropertyAccessor(
            strong_getter.id(),
        )),
        origin_record(DefinitionOriginSubject::PropertyAccessor(
            structural_getter.id(),
        )),
        origin_record(DefinitionOriginSubject::PropertyAccessor(
            structural_setter.id(),
        )),
        origin_record(DefinitionOriginSubject::PropertyAccessor(
            generic_getter.id(),
        )),
    ];
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_types(vec![
            CborIdentityRecord::from_key(object_source).unwrap(),
            text,
        ])
        .unwrap();
    foundation
        .set_properties(vec![strong_property.clone(), structural_property.clone()])
        .unwrap();
    foundation
        .set_extension_properties(vec![generic_property.clone()])
        .unwrap();
    foundation.set_object_values(vec![object_value]).unwrap();
    foundation
        .set_property_accessors(vec![
            strong_getter,
            structural_getter.clone(),
            structural_setter,
            generic_getter,
        ])
        .unwrap();
    foundation
        .set_exact_types(vec![
            tuple_exact_record,
            object_exact_record,
            text_exact_record,
        ])
        .unwrap();
    foundation
        .set_export_bindings(vec![
            object_type_binding.clone(),
            object_value_binding,
            strong_binding,
            structural_binding,
            generic_binding,
        ])
        .unwrap();
    foundation.set_definition_origins(origins.clone()).unwrap();
    let direct = CanonicalDirectPublicSurfaceV1::try_new(foundation_bindings(
        &surface,
        object_type_binding.id(),
    ))
    .unwrap();

    Fixture {
        foundation,
        direct,
        surface,
        origins,
        object_type,
        text_type,
        strong_property: strong_property.id(),
        structural_property: structural_property.id(),
        structural_getter: structural_getter.id(),
        object_type_binding: object_type_binding.id(),
        object_exact,
        text_exact,
        tuple_exact,
    }
}

fn foundation_bindings(
    surface: &CoreValueTargetSurfaceV1,
    object_type_binding: PersistentExportBindingId,
) -> Vec<PersistentExportBindingId> {
    surface
        .targets()
        .iter()
        .map(CoreValueTargetV1::binding)
        .chain(std::iter::once(object_type_binding))
        .collect()
}

fn nominal(name: &str, kind: SourceNominalKind, type_parameter_count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        type_parameter_count,
    )
}

fn property(name: &str) -> CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::property(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn extension_property(
    name: &str,
) -> CborIdentityRecord<PersistentExtensionPropertyId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::extension_property(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        1,
        SignatureTypeKey::Binder { depth: 0, index: 0 },
    ))
    .unwrap()
}

fn accessor(
    owner: IdentityPropertyOwner,
    role: AccessorRole,
) -> CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey> {
    CborIdentityRecord::from_key(PropertyAccessorKey::new(owner, role)).unwrap()
}

fn exact(key: ExactTypeKey) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(key).unwrap()
}

fn type_binding(
    declaration: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    binding(declaration, BindingTarget::type_name(declaration).unwrap())
}

fn value_binding(
    declaration: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    let target = match declaration.declaration_kind() {
        SourceDeclarationKind::Object => BindingTarget::object_value(declaration).unwrap(),
        SourceDeclarationKind::Property => BindingTarget::property(declaration).unwrap(),
        SourceDeclarationKind::ExtensionProperty => {
            BindingTarget::extension_property(declaration).unwrap()
        }
        _ => panic!("fixture value declaration has the wrong kind"),
    };
    binding(declaration, target)
}

fn binding(
    declaration: &SourceDeclarationKey,
    target: BindingTarget,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    let DeclarationName::Named(name) = declaration.name() else {
        panic!("fixture declaration must be named")
    };
    CborIdentityRecord::from_key(ExportBindingKey::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        name.clone(),
        target,
    ))
    .unwrap()
}

fn origin_record(subject: DefinitionOriginSubject) -> DefinitionOriginRecord {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/core.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    let origin = DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap();
    DefinitionOriginRecord::new(subject, origin)
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
