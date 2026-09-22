use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey,
    GcEffect as IdentityGcEffect, ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, PersistentSymbolKey, PersistentTypeId,
    ScoopAbiReturn as CanonicalAbiReturn, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind, StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::{AbiReturn, CallingConvention, GcEffect, ScoopAbiSignature};

#[test]
fn callable_binds_target_symbol_definition_and_root_protocol() {
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        core_site(),
        CanonicalIdentifier::new("println").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let target = StrongCallableDefinitionOwner::Function(function);
    let unit = exact_type("Unit", SourceNominalKind::Object);
    let canonical_signature = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
        Vec::new(),
        CanonicalAbiReturn::unit_void(),
        IdentityGcEffect::Managed,
    )
    .unwrap();
    let selected = crate::SelectedDependencyLirCallableV1::new(
        ConeIdentity::CORE,
        scoop_identity::DependencyCallableDeclarationId::Function(function),
        target,
        canonical_signature,
        CallingConvention::Cdecl,
        crate::ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap();
    let set = crate::SelectedExternalLirSet::empty(ConeIdentity::SINGLE_FILE)
        .with_initialization_cycle(selected)
        .unwrap();
    let callable = set
        .callable(set.initialization_cycle().unwrap())
        .unwrap()
        .materialize(ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        ))
        .unwrap();

    assert_eq!(callable.target(), target);
    assert_eq!(callable.calling_convention(), CallingConvention::Cdecl);
    assert_eq!(callable.gc_effect(), GcEffect::Managed);
    assert_eq!(
        callable.expected_symbol().key(),
        PersistentSymbolKey::CallableBody(callable.body())
    );
    let expected = ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::callable_body(callable.body()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(callable.required_definition(), expected);
    assert!(
        callable
            .expected_symbol()
            .symbol()
            .as_str()
            .starts_with("scoop$1$cb$")
    );
}

fn exact_type(name: &str, kind: SourceNominalKind) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        core_site(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    );
    let ty = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(ty)).unwrap()
}

fn core_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
