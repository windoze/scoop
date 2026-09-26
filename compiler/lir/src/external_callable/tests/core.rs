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
fn callable_binds_actual_provider_target_symbol_definition_and_root_protocol() {
    let ordinary = scoop_identity::ConeCoordinate::new("tests", "initialization", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for provider in [ConeIdentity::CORE, ordinary] {
        check_callable(provider);
    }
}

fn check_callable(provider: ConeIdentity) {
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site(provider),
        CanonicalIdentifier::new("println").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let target = StrongCallableDefinitionOwner::Function(function);
    let unit = exact_type("Unit", SourceNominalKind::Object);
    assert_ne!(provider, ConeIdentity::SINGLE_FILE);
    let canonical_signature = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
        Vec::new(),
        CanonicalAbiReturn::unit_void(),
        IdentityGcEffect::Managed,
    )
    .unwrap();
    let selected = crate::SelectedDependencyLirCallableV1::new(
        provider,
        scoop_identity::DependencyCallableDeclarationId::Function(function),
        target,
        canonical_signature,
        CallingConvention::Cdecl,
        crate::ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap();
    let set = crate::SelectedExternalLirSet::try_from_callables(
        ConeIdentity::SINGLE_FILE,
        vec![selected],
    )
    .unwrap();
    let callable = set
        .callable(
            set.callable_for(
                provider,
                scoop_identity::DependencyCallableDeclarationId::Function(function),
            )
            .unwrap(),
        )
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
            provider,
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
        site(ConeIdentity::CORE),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    );
    let ty = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(ty)).unwrap()
}

fn site(provider: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
