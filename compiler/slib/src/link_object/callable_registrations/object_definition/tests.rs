use scoop_identity::PersistentStaticStorageId;
use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    NonEmptyVec, OdrGroupId, OdrMemberDiscriminator, OdrMemberId, OdrMemberKey, OdrMemberRole,
    PackagePath, PersistentExactTypeId, PersistentFunctionId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, SpecializationKey,
    StrongCallableDefinitionOwner,
};
use scoop_lir::ExternalStrongShapeSubjectV1;
use scoop_wire::encode_runtime;

use super::*;

#[test]
fn storage_and_initialization_targets_preserve_the_original_definition_identity() {
    use scoop_identity::{InitializationUnitKey, PersistentInitializationUnitId, StaticStorageKey};

    let provider = ConeCoordinate::new("test", "provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let site = SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let object = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new("State").unwrap(),
        SourceNominalKind::Object,
        0,
    ))
    .unwrap();
    let storage =
        PersistentStaticStorageId::from_key(&StaticStorageKey::singleton_published_root(object))
            .unwrap();
    let unit =
        PersistentInitializationUnitId::from_key(&InitializationUnitKey::Object(object)).unwrap();
    for (subject, entity_tag, identity, role_tag) in [
        (
            ExternalStrongShapeSubjectV1::StaticStorage(storage),
            2_u32,
            storage.as_array(),
            2_u32,
        ),
        (
            ExternalStrongShapeSubjectV1::StaticStorageRegistration(storage),
            2,
            storage.as_array(),
            11,
        ),
        (
            ExternalStrongShapeSubjectV1::InitializationCell(unit),
            9,
            unit.as_array(),
            9,
        ),
        (
            ExternalStrongShapeSubjectV1::InitializationRegistration(unit),
            9,
            unit.as_array(),
            13,
        ),
    ] {
        let mut expected = 1_u32.to_le_bytes().to_vec();
        expected.extend_from_slice(&entity_tag.to_le_bytes());
        expected.extend_from_slice(identity);
        expected.extend_from_slice(&role_tag.to_le_bytes());
        let requirement =
            CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong { provider, subject };
        assert_eq!(
            encode_runtime(&requirement).unwrap(),
            expected,
            "{subject:?}"
        );
    }
}

#[test]
fn local_and_dependency_callable_targets_have_the_same_runtime_encoding() {
    let provider = ConeCoordinate::new("test", "provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("called").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let target = StrongCallableDefinitionOwner::Function(
        PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
    );
    let requirement = CanonicalObjectDefinitionRequirementV1::DependencyStrong { provider, target };

    let body = scoop_identity::PersistentCallableBodyId::from_key(
        &scoop_identity::CallableBodyKey::strong(target),
    )
    .unwrap();
    let local = CanonicalObjectDefinitionRequirementV1::Legacy(
        FinalUndefinedSymbolRequirementV1::IntraConeStrong {
            owner: StrongDefinitionOwnerV1::new(
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
            )
            .unwrap(),
        },
    );
    let expected = encode_runtime(&local).unwrap();
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);

    let subject = ExternalStrongShapeSubjectV1::Callable(
        scoop_identity::CallableDefinitionOwner::Strong(target),
    );
    let requirement =
        CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong { provider, subject };
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);
}

#[test]
fn descriptor_targets_use_definition_identity_without_reusing_retired_service_tags() {
    let provider = ConeCoordinate::new("test", "provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let site = SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let nominal = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new("Value").unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap();
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
    let requirement = CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong {
        provider,
        subject: ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
    };

    let mut expected = 1_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(&4_u32.to_le_bytes());
    expected.extend_from_slice(exact.as_array());
    expected.extend_from_slice(&4_u32.to_le_bytes());
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);
    assert_eq!(
        encode_runtime(&CanonicalObjectRelocationV1::intra_cone_type_descriptor(
            16, exact
        ))
        .unwrap(),
        encode_runtime(&CanonicalObjectRelocationV1::dependency_target(
            16,
            provider,
            ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        ))
        .unwrap(),
    );

    let tuple = PersistentExactTypeId::from_key(&ExactTypeKey::Tuple(NonEmptyVec::from_first(
        exact,
        [exact],
    )))
    .unwrap();
    let group =
        OdrGroupId::from_key(&SpecializationKey::StructuralType { exact_type: tuple }).unwrap();
    let member = OdrMemberId::from_key(
        &OdrMemberKey::new(
            group,
            OdrMemberRole::TypeDescriptor,
            OdrMemberDiscriminator::ExactType(tuple),
        )
        .unwrap(),
    )
    .unwrap();
    let requirement = CanonicalObjectDefinitionRequirementV1::Legacy(
        FinalUndefinedSymbolRequirementV1::OdrMember { member },
    );
    let mut expected = 14_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(member.as_array());
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);
}
