use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    LexicalCallableParent, LexicalCallableRole, PackagePath, PersistentFunctionId,
    SourceDeclarationKey, SourceDeclarationSite, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{DecodeLimits, ResourceKind};

use super::*;

mod derived_parameters;

#[test]
fn generated_origin_walk_obeys_the_semantic_depth_limit() {
    let function_key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("origin_depth_root").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
    let parent_key = GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::function(function),
        role: LexicalCallableRole::LambdaBody,
        path: StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
            [],
        ),
    };
    let parent = CborIdentityRecord::from_key(parent_key).unwrap();
    let child = CborIdentityRecord::from_key(GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::from_generated_key(parent.key()).unwrap(),
        role: LexicalCallableRole::AnonymousFunctionBody,
        path: StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 1),
            [],
        ),
    })
    .unwrap();
    let child_id = child.id();
    let records = vec![parent, child];
    let path = WirePath::root().field(29);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });
    let mut requirements = OriginRequirements::new(&records, 0, &mut meter, &path).unwrap();

    let error = requirements
        .generated_subject(child_id, &mut meter, &path)
        .unwrap_err();
    assert!(matches!(
        error,
        HirFoundationValidationError::Resource(ref error)
            if error.kind() == &WireErrorKind::LimitExceeded {
                resource: ResourceKind::SemanticRecursion,
                limit: 1,
                observed: 2,
            }
    ));
}
