use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, DependencyCallableDeclarationId, Effect,
    ExactCallableSignature, ExactTypeKey, GcEffect as CanonicalGcEffect, ObjectDefinitionPlanKey,
    PackagePath, PersistentExactTypeId, PersistentFunctionId, ScoopAbiReturn as CanonicalAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
};

use super::*;
use crate::{
    AbiReturn, CallingConvention, DependencyExternalCallableRootPlanV1, ScoopAbiSignature,
    SelectedDependencyLirCallableV1,
};

#[test]
fn selected_bridge_materializes_one_typed_dependency_external() {
    let provider = cone("provider");
    let selected = selected(provider, CallingConvention::Cdecl);
    let external = selected
        .materialize(ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        ))
        .unwrap();

    assert_eq!(external.provider(), provider);
    assert_eq!(
        external.legacy_declaration(),
        Some(selected.bridge().declaration())
    );
    assert_eq!(external.target(), selected.bridge().target());
    assert_eq!(external.gc_effect(), GcEffect::Managed);
    assert_eq!(
        external.expected_symbol(),
        selected.bridge().expected_symbol()
    );
    assert_eq!(
        external.required_definition(),
        selected.bridge().required_definition()
    );
    assert_eq!(
        external.required_definition(),
        scoop_identity::ObjectDefinitionPlanId::from_key(
            &ObjectDefinitionPlanKey::strong(
                provider,
                scoop_identity::StrongDefinitionEntity::callable_body(external.body()),
                scoop_identity::StrongDefinitionRole::CallableBody,
            )
            .unwrap(),
        )
        .unwrap()
    );
}

#[test]
fn materialization_rejects_physical_abi_drift() {
    let selected = selected(cone("provider"), CallingConvention::Cdecl);
    assert!(matches!(
        selected.materialize(ScoopAbiSignature::new(
            vec![crate::AbiArgument::Direct(
                crate::AbiValue::new(
                    crate::LirType::I64,
                    crate::AbiNonZeroLayout::new(8, 8).unwrap(),
                    crate::RefScan::None,
                )
                .unwrap()
            )],
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        )),
        Err(ExternalCallableBuildError::AbiArgumentCount {
            expected: 0,
            actual: 1
        })
    ));
}

fn selected(
    provider: ConeIdentity,
    calling_convention: CallingConvention,
) -> SelectedDependencyLirCallableV1 {
    let source = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("run").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&source).unwrap();
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    let exact = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
    let canonical = CanonicalScoopAbiFunctionSignature::new(
        exact,
        Vec::new(),
        CanonicalAbiReturn::unit_void(),
        CanonicalGcEffect::Managed,
    )
    .unwrap();
    SelectedDependencyLirCallableV1::new(
        provider,
        DependencyCallableDeclarationId::Function(function),
        StrongCallableDefinitionOwner::Function(function),
        canonical,
        calling_convention,
        DependencyExternalCallableRootPlanV1::ManagedStatepoint,
    )
    .unwrap()
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("tests", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
