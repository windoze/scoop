use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, DeclarationName, DeclarationScope,
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOwnerChain, ExportBindingKey, NonEmptyVec,
    NormalizedSourcePath, PackagePath, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn type_surface_has_fixed_wire_for_every_definition_and_capability_variant() {
    let fixture = fixture();
    let bytes = encode(&fixture.surface).unwrap();
    assert_eq!(
        hex(&bytes),
        "84a301582015ec522c9d344aa84f567ab6f0a3f2400b214cece600334ed3188afc5f9eca2f02a20003015820ca9b9e2ec29988c860b531fa7bc3ce9a342f4a3b671e02afe233aea6bfe2d06d03a20001015820ba3a2277b6c2dc18604fd5b344ae9d9403abcbe3d55829f2a0d07cddd2503b36a30158202721d4e2121a727a6784d83c68bd0bd133c8daae7abcd2eeb641bb7377af3ade02a20002015820f1ea16890af312cfb08246a88afa5c7b5748c30c11bb1c83d79476c0c4d6faf503a200030101a30158208f5ec5991dde8c40ada0687d776a127cd1e3ad38a288c38b41fbe30cd921a2a902a200010158201f735aeecea8726a3e13676d04618097f05e2b7bf0f3e428876aa2e014e0b2b503a20001015820ba3a2277b6c2dc18604fd5b344ae9d9403abcbe3d55829f2a0d07cddd2503b36a3015820eb12d5157a58a1b34f4c21918ab9300d341298079079ad8f72baf624fb27970c02a2000301582031a25e94bca418d9da18a7c2abef2429818fe35d2ba7447c69e7b7fe4b6575b003a20002015820e903528dd5072ab4d40edeae0b51a27fd456c72411201ec669867b52539545b7"
    );

    assert_eq!(
        decode(&fixture.surface).validate_against(&fixture.foundation, &fixture.direct),
        Ok(fixture.surface)
    );
}

#[test]
fn type_surface_reader_rejects_unknown_missing_and_extra_fields() {
    for bytes in [
        vec![0x81, 0xa2, 0x01, 0x40, 0x02, 0xa1, 0x00, 0x01],
        vec![
            0x81, 0xa4, 0x01, 0x40, 0x02, 0xa1, 0x00, 0x01, 0x03, 0xa0, 0x04, 0x00,
        ],
    ] {
        assert!(
            decode_canonical::<DecodedCoreTypeTargetSurfaceV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa1, 0x00, 0x04],
        vec![0xa1, 0x00, 0x01],
        vec![0xa3, 0x00, 0x01, 0x01, 0x40, 0x02, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreTypeDefinitionV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa1, 0x00, 0x04],
        vec![0xa1, 0x00, 0x01],
        vec![0xa3, 0x00, 0x01, 0x01, 0x40, 0x02, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreHirTypeCapabilityV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

#[test]
fn type_surface_requires_canonical_total_direct_type_coverage() {
    let fixture = fixture();
    let targets = fixture.surface.targets().to_vec();

    let missing = raw_surface(&targets[..targets.len() - 1]);
    assert_eq!(
        missing.validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::Coverage {
            expected: targets.len(),
            actual: targets.len() - 1,
        })
    );

    let mut reversed = targets.clone();
    reversed.reverse();
    assert!(matches!(
        raw_surface(&reversed).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::NonCanonicalOrder { .. })
    ));

    let mut duplicate = targets.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(matches!(
        raw_surface(&duplicate).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::DuplicateBinding { .. })
    ));

    let unknown_binding = type_binding(alias("UnknownAlias").key()).id();
    let unknown_direct = CanonicalDirectPublicSurfaceV1::try_new(vec![unknown_binding]).unwrap();
    assert!(matches!(
        decode(&fixture.surface).validate_against(&fixture.foundation, &unknown_direct),
        Err(CoreTypeTargetSurfaceValidationError::UnknownDirectSurfaceBinding(binding))
            if binding == unknown_binding
    ));
}

#[test]
fn type_surface_replays_definition_origin_exact_target_and_capability() {
    let fixture = fixture();
    let concrete_index = target_index(&fixture, |definition| {
        matches!(definition, CoreTypeDefinitionV1::Type(_))
    });
    let generic_index = target_index(&fixture, |definition| {
        matches!(definition, CoreTypeDefinitionV1::GenericType(_))
    });
    let nominal_alias_index = target_index(&fixture, |definition| {
        definition == CoreTypeDefinitionV1::TypeAlias(fixture.nominal_alias)
    });
    let structural_alias_index = target_index(&fixture, |definition| {
        definition == CoreTypeDefinitionV1::TypeAlias(fixture.structural_alias)
    });

    let mut wrong_definition = fixture.surface.targets().to_vec();
    wrong_definition[concrete_index].definition = wrong_definition[generic_index].definition;
    assert!(matches!(
        raw_surface(&wrong_definition).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::DefinitionMismatch(_))
    ));

    let mut without_origin = fixture.foundation.clone();
    without_origin.set_definition_origins(Vec::new()).unwrap();
    assert!(matches!(
        decode(&fixture.surface).validate_against(&without_origin, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::MissingDefinitionOrigin(_))
    ));

    let mut wrong_concrete_exact = fixture.surface.targets().to_vec();
    wrong_concrete_exact[concrete_index].capability =
        CoreHirTypeCapabilityV1::ParamFreeStrong(fixture.tuple_exact);
    assert!(matches!(
        raw_surface(&wrong_concrete_exact).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::ExactTargetMismatch(_))
    ));

    let mut wrong_nominal_alias_branch = fixture.surface.targets().to_vec();
    wrong_nominal_alias_branch[nominal_alias_index].capability =
        CoreHirTypeCapabilityV1::StructuralUnavailable(fixture.nominal_exact);
    assert!(matches!(
        raw_surface(&wrong_nominal_alias_branch)
            .validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::CapabilityMismatch(_))
    ));

    let mut wrong_structural_alias_branch = fixture.surface.targets().to_vec();
    wrong_structural_alias_branch[structural_alias_index].capability =
        CoreHirTypeCapabilityV1::ParamFreeStrong(fixture.tuple_exact);
    assert!(matches!(
        raw_surface(&wrong_structural_alias_branch)
            .validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::CapabilityMismatch(_))
    ));

    let mut wrong_generic_count = fixture.surface.targets().to_vec();
    wrong_generic_count[generic_index].capability = CoreHirTypeCapabilityV1::GenericUnavailable {
        type_parameter_count: 2,
    };
    assert!(matches!(
        raw_surface(&wrong_generic_count).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::CapabilityMismatch(_))
    ));

    let unknown_type = nominal("Unknown");
    let unknown_exact =
        CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Nominal(
            PersistentTypeId::from_source_declaration(unknown_type.key()).unwrap(),
        ))
        .unwrap();
    let mut unknown_exact_target = fixture.surface.targets().to_vec();
    unknown_exact_target[nominal_alias_index].capability =
        CoreHirTypeCapabilityV1::ParamFreeStrong(unknown_exact.id());
    assert!(matches!(
        raw_surface(&unknown_exact_target).validate_against(&fixture.foundation, &fixture.direct),
        Err(CoreTypeTargetSurfaceValidationError::UnknownExactType(_))
    ));
}

fn decode(surface: &CoreTypeTargetSurfaceV1) -> DecodedCoreTypeTargetSurfaceV1 {
    decode_canonical(&encode(surface).unwrap(), DecodeLimits::default()).unwrap()
}

fn raw_surface(targets: &[CoreTypeTargetV1]) -> DecodedCoreTypeTargetSurfaceV1 {
    struct RawSurface<'a>(&'a [CoreTypeTargetV1]);

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

fn target_index(fixture: &Fixture, predicate: impl Fn(CoreTypeDefinitionV1) -> bool) -> usize {
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
    surface: CoreTypeTargetSurfaceV1,
    nominal_alias: PersistentTypeAliasId,
    structural_alias: PersistentTypeAliasId,
    nominal_exact: PersistentExactTypeId,
    tuple_exact: PersistentExactTypeId,
}

fn fixture() -> Fixture {
    let concrete = nominal("Text");
    let concrete_id = concrete.id();
    let generic = generic_nominal("Box", 1);
    let generic_id = generic.id();
    let nominal_alias = alias("TextAlias");
    let structural_alias = alias("Pair");

    let nominal_exact_record = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(
        ExactTypeKey::Nominal(concrete_id),
    )
    .unwrap();
    let tuple_exact_record =
        CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Tuple(
            NonEmptyVec::new(vec![nominal_exact_record.id(), nominal_exact_record.id()]).unwrap(),
        ))
        .unwrap();
    let nominal_exact = nominal_exact_record.id();
    let tuple_exact = tuple_exact_record.id();

    let concrete_binding = type_binding(concrete.key());
    let generic_binding = type_binding(generic.key());
    let nominal_alias_binding = type_binding(nominal_alias.key());
    let structural_alias_binding = type_binding(structural_alias.key());

    let surface = CoreTypeTargetSurfaceV1::try_new(vec![
        CoreTypeTargetV1 {
            binding: structural_alias_binding.id(),
            definition: CoreTypeDefinitionV1::TypeAlias(structural_alias.id()),
            capability: CoreHirTypeCapabilityV1::StructuralUnavailable(tuple_exact),
        },
        CoreTypeTargetV1 {
            binding: nominal_alias_binding.id(),
            definition: CoreTypeDefinitionV1::TypeAlias(nominal_alias.id()),
            capability: CoreHirTypeCapabilityV1::ParamFreeStrong(nominal_exact),
        },
        CoreTypeTargetV1 {
            binding: generic_binding.id(),
            definition: CoreTypeDefinitionV1::GenericType(generic_id),
            capability: CoreHirTypeCapabilityV1::GenericUnavailable {
                type_parameter_count: 1,
            },
        },
        CoreTypeTargetV1 {
            binding: concrete_binding.id(),
            definition: CoreTypeDefinitionV1::Type(concrete_id),
            capability: CoreHirTypeCapabilityV1::ParamFreeStrong(nominal_exact),
        },
    ])
    .unwrap();

    let mut foundation = CanonicalHirFoundation::empty();
    foundation.set_types(vec![concrete]).unwrap();
    foundation.set_generic_types(vec![generic]).unwrap();
    foundation
        .set_type_aliases(vec![nominal_alias.clone(), structural_alias.clone()])
        .unwrap();
    foundation
        .set_exact_types(vec![tuple_exact_record, nominal_exact_record])
        .unwrap();
    foundation
        .set_export_bindings(vec![
            structural_alias_binding,
            nominal_alias_binding,
            generic_binding,
            concrete_binding,
        ])
        .unwrap();
    foundation
        .set_definition_origins(vec![
            origin_record(DefinitionOriginSubject::Type(concrete_id)),
            origin_record(DefinitionOriginSubject::GenericType(generic_id)),
            origin_record(DefinitionOriginSubject::TypeAlias(nominal_alias.id())),
            origin_record(DefinitionOriginSubject::TypeAlias(structural_alias.id())),
        ])
        .unwrap();
    let direct = CanonicalDirectPublicSurfaceV1::try_new(
        surface
            .targets()
            .iter()
            .map(CoreTypeTargetV1::binding)
            .collect(),
    )
    .unwrap();

    Fixture {
        foundation,
        direct,
        surface,
        nominal_alias: nominal_alias.id(),
        structural_alias: structural_alias.id(),
        nominal_exact,
        tuple_exact,
    }
}

fn nominal(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap()
}

fn generic_nominal(
    name: &str,
    type_parameter_count: u32,
) -> CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        type_parameter_count,
    ))
    .unwrap()
}

fn alias(name: &str) -> CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::type_alias(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn type_binding(
    declaration: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    let target = if declaration.declaration_kind().is_nominal() {
        BindingTarget::type_name(declaration).unwrap()
    } else {
        BindingTarget::type_alias(declaration).unwrap()
    };
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
