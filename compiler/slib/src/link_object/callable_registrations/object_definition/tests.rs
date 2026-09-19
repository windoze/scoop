use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentExactTypeId, PersistentFunctionId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
};
use scoop_lir::ExternalStrongShapeSubjectV1;
use scoop_wire::encode_runtime;

use super::*;

#[test]
fn dependency_targets_have_distinct_stable_runtime_tags() {
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

    let mut expected = 11_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(provider.as_array());
    expected.extend_from_slice(&encode_runtime(&target).unwrap());
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);

    let subject = ExternalStrongShapeSubjectV1::Callable(target);
    let requirement =
        CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong { provider, subject };
    let mut expected = 12_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(provider.as_array());
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&encode_runtime(&target).unwrap());
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);
}

#[test]
fn dependency_shape_target_encodes_non_callable_payload_as_typed_raw_id() {
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

    let mut expected = 12_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(provider.as_array());
    expected.extend_from_slice(&4_u32.to_le_bytes());
    expected.extend_from_slice(exact.as_array());
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);
}
