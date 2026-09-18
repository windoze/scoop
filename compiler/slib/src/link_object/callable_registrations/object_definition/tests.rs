use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, PackagePath,
    PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner,
};
use scoop_wire::encode_runtime;

use super::*;

#[test]
fn dependency_strong_target_has_a_distinct_stable_runtime_tag() {
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
}
