use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey, GcEffect, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, PersistentTypeId, ScoopAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CallingConvention, ExternalCallableRootPlan, LirTargetProfile, SelectedDependencyLirCallableV1,
    StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
};

use super::*;
use crate::link_object::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, LinkDefinitionOwnerV1,
    verify_current_cone_strong_relocation_closure_v1,
};

#[test]
fn resolves_external_bridges_against_the_actual_provider_owner() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "coreConsumer");
    let bridge = core_callable_bridge("required");
    let expected_name = bridge_name(&bridge);
    let member = verified_member_with_undefined(&source, &expected_name);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge.clone()]).unwrap();
    let dependency_owners = core_callable_owner_set("required");
    let expected_owner = super::dependencies::expected_owner(&bridge).unwrap();
    let expected_member = dependency_owners
        .owners()
        .iter()
        .find(|owner| owner.owner() == LinkDefinitionOwnerV1::StrongDefinition(expected_owner))
        .unwrap()
        .member();

    let verified = verify_dependency_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        surface,
        std::slice::from_ref(&dependency_owners),
    )
    .unwrap();

    assert_eq!(verified.producer(), producer);
    assert_eq!(
        verified.dependency_owners(),
        std::slice::from_ref(&dependency_owners)
    );
    assert_eq!(verified.external_requirements().len(), 1);
    let requirement = &verified.external_requirements()[0];
    assert_eq!(requirement.bridge(), &bridge);
    assert_eq!(requirement.owner(), expected_owner);
    assert_eq!(requirement.member(), expected_member);
    assert_eq!(requirement.use_site().symbol(), expected_name);
    assert!(verified.remaining_external_candidates().is_empty());
}

#[test]
fn preserves_native_external_candidates() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "nativeConsumer");
    let member = verified_member_with_undefined(&source, b"_native_symbol");
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let verified = verify_dependency_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        surface,
        &[],
    )
    .unwrap();

    assert!(verified.external_requirements().is_empty());
    assert_eq!(verified.remaining_external_candidates().len(), 1);
    assert_eq!(
        verified.remaining_external_candidates()[0].symbol(),
        b"_native_symbol"
    );
}

#[test]
fn rejects_unused_callable_bridges_and_cross_producer_surfaces() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "unusedBridgeConsumer");
    let member = verified_member_without_relocations(&source);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let bridge = core_callable_bridge("unusedBridge");
    let expected_name = bridge_name(&bridge);
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge.clone()]).unwrap();
    assert_eq!(
        verify_dependency_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            &[core_callable_owner_set("unusedBridge")],
        ),
        Err(
            CrossConeStrongRequirementValidationError::UnusedExternalCallableBridge {
                name: expected_name,
            }
        )
    );

    let source = fixture_for_producer(producer, "producerMismatch");
    let member = verified_member_without_relocations(&source);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::CORE, Vec::new()).unwrap();
    assert_eq!(
        verify_dependency_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            &[core_callable_owner_set("Mismatch")],
        ),
        Err(
            CrossConeStrongRequirementValidationError::ConsumerMismatch {
                object: producer,
                bridge: ConeIdentity::CORE,
            }
        )
    );
}

fn core_callable_bridge(name: &str) -> StrongExternalLirBridgeV1 {
    callable_bridge(ConeIdentity::CORE, name)
}

fn callable_bridge(provider: ConeIdentity, name: &str) -> StrongExternalLirBridgeV1 {
    let target = StrongCallableDefinitionOwner::Function(function(provider, name));
    let unit = core_exact_type("Unit");
    StrongExternalLirBridgeV1::Callable(Box::new(
        SelectedDependencyLirCallableV1::new(
            provider,
            scoop_identity::DependencyCallableDeclarationId::Function(function(provider, name)),
            target,
            CanonicalScoopAbiFunctionSignature::new(
                ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
                Vec::new(),
                ScoopAbiReturn::unit_void(),
                GcEffect::Managed,
            )
            .unwrap(),
            CallingConvention::Cdecl,
            ExternalCallableRootPlan::ManagedStatepoint,
        )
        .unwrap(),
    ))
}

#[test]
fn rejects_missing_symbol_and_self_dependency() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "missingCoreOwner");
    let bridge = core_callable_bridge("required");
    let expected_name = bridge_name(&bridge);
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &source,
            &expected_name,
        )])
        .unwrap();
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge.clone()]).unwrap();
    let expected_owner = super::dependencies::expected_owner(&bridge).unwrap();
    assert_eq!(
        verify_dependency_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            &[core_callable_owner_set("Different")],
        ),
        Err(
            CrossConeStrongRequirementValidationError::MissingDependencyOwner {
                provider: ConeIdentity::CORE,
                name: expected_name,
                owner: expected_owner,
            }
        )
    );

    let source = fixture_for_producer(producer, "nonCoreOwnerAuthority");
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&source),
    ])
    .unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let non_core = fixture_for_producer(producer, "notCore");
    let non_core_strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&non_core),
    ])
    .unwrap();
    let non_dependency_owners =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&non_core_strong)
            .unwrap();
    assert_eq!(
        verify_dependency_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            &[non_dependency_owners],
        ),
        Err(CrossConeStrongRequirementValidationError::SelfDependency { provider: producer })
    );
}

fn bridge_name(bridge: &StrongExternalLirBridgeV1) -> Vec<u8> {
    let request = match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => bridge.bridge().expected_symbol(),
    };
    LirTargetProfile::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(request.symbol().as_str())
        .into_bytes()
}

fn core_exact_type(name: &str) -> PersistentExactTypeId {
    exact_type(ConeIdentity::CORE, name)
}

fn exact_type(provider: ConeIdentity, name: &str) -> PersistentExactTypeId {
    let site = SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let source = SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let ty = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(ty)).unwrap()
}

fn function(provider: ConeIdentity, name: &str) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn core_callable_owner_set(name: &str) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    callable_owners(ConeIdentity::CORE, name)
}

fn callable_owners(provider: ConeIdentity, name: &str) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    let callable = fixture_for_producer(provider, name);
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&callable),
    ])
    .unwrap();
    CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap()
}

mod providers;
