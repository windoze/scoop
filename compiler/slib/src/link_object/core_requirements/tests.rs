use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey, GcEffect, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, PersistentTypeId, ScoopAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CallingConvention, CoreExternalCallableRootPlan, LirTargetProfile,
    StrongExternalCallableBridgeV1, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
    StrongExternalTypeDescriptorBridgeV1,
};

use super::super::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use super::super::symbol_verification::tests::{
    fixture_for_producer, fixture_for_type_descriptor, fixture_for_type_registration,
};
use super::*;
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, verify_current_cone_strong_relocation_closure_v1,
};

#[test]
fn resolves_core_bridges_against_the_actual_core_owner() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "coreConsumer");
    let target = core_exact_type("String");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(target).unwrap(),
    );
    let expected_name = bridge_name(&bridge);
    let member = verified_member_with_undefined(&source, &expected_name);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge.clone()]).unwrap();
    let core_owners = core_owner_set(target);
    let expected_owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(target),
        StrongDefinitionRole::TypeDescriptor,
    )
    .unwrap();
    let expected_member = core_owners
        .owners()
        .iter()
        .find(|owner| owner.owner() == LinkDefinitionOwnerV1::StrongDefinition(expected_owner))
        .unwrap()
        .member();

    let verified = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        surface,
        core_owners.clone(),
    )
    .unwrap();

    assert_eq!(verified.producer(), producer);
    assert_eq!(verified.core_owners(), &core_owners);
    assert_eq!(verified.core_requirements().len(), 1);
    let requirement = &verified.core_requirements()[0];
    assert_eq!(requirement.bridge(), &bridge);
    assert_eq!(requirement.owner(), expected_owner);
    assert_eq!(requirement.core_member(), expected_member);
    assert_eq!(requirement.use_site().symbol(), expected_name);
    assert!(verified.remaining_external_candidates().is_empty());
}

#[test]
fn resolves_the_type_registration_support_of_a_core_descriptor_bridge() {
    let producer = ConeIdentity::SINGLE_FILE;
    let target = core_exact_type("SupportedString");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(target).unwrap(),
    );
    let descriptor_source = fixture_for_producer(producer, "coreDescriptorConsumer");
    let registration_source = fixture_for_producer(producer, "coreRegistrationConsumer");
    let registration_name = type_registration_name(target);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_undefined(&descriptor_source, &bridge_name(&bridge)),
        verified_member_with_undefined(&registration_source, &registration_name),
    ])
    .unwrap();
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge.clone()]).unwrap();

    let verified = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        surface,
        core_owner_set(target),
    )
    .unwrap();

    assert_eq!(verified.core_requirements().len(), 2);
    let support = verified
        .core_requirements()
        .iter()
        .find(|requirement| requirement.use_site().symbol() == registration_name)
        .unwrap();
    assert_eq!(support.bridge(), &bridge);
    assert_eq!(
        support.owner(),
        StrongDefinitionOwnerV1::new(
            StrongDefinitionEntity::exact_type(target),
            StrongDefinitionRole::TypeRegistration,
        )
        .unwrap()
    );
    assert!(verified.remaining_external_candidates().is_empty());
}

#[test]
fn rejects_a_core_descriptor_bridge_without_its_registration_support_owner() {
    let producer = ConeIdentity::SINGLE_FILE;
    let target = core_exact_type("MissingRegistrationSupport");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(target).unwrap(),
    );
    let source = fixture_for_producer(producer, "missingRegistrationSupportConsumer");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &source,
            &bridge_name(&bridge),
        )])
        .unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap();
    let expected_name = type_registration_name(target);
    let expected_owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(target),
        StrongDefinitionRole::TypeRegistration,
    )
    .unwrap();

    assert_eq!(
        verify_core_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            descriptor_only_core_owner_set(target),
        ),
        Err(CoreStrongRequirementValidationError::MissingCoreOwner {
            name: expected_name,
            owner: expected_owner,
        })
    );
}

#[test]
fn preserves_non_core_external_candidates() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "nativeConsumer");
    let member = verified_member_with_undefined(&source, b"_native_symbol");
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let verified = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        surface,
        core_owner_set(core_exact_type("String")),
    )
    .unwrap();

    assert!(verified.core_requirements().is_empty());
    assert_eq!(verified.remaining_external_candidates().len(), 1);
    assert_eq!(
        verified.remaining_external_candidates()[0].symbol(),
        b"_native_symbol"
    );
}

#[test]
fn permits_metadata_only_descriptor_authorities() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "metadataOnlyDescriptor");
    let member = verified_member_without_relocations(&source);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let target = core_exact_type("Unit");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(target).unwrap(),
    );
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap();
    let verified = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        surface,
        core_owner_set(target),
    )
    .unwrap();
    assert!(verified.core_requirements().is_empty());
    assert_eq!(verified.external_bridges().bridges().len(), 1);
}

#[test]
fn rejects_unused_callable_bridges_and_cross_producer_surfaces() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "unusedBridgeConsumer");
    let member = verified_member_without_relocations(&source);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let bridge = core_callable_bridge("unusedBridge");
    let expected_name = bridge_name(&bridge);
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap();
    assert_eq!(
        verify_core_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            core_callable_owner_set("unusedBridge"),
        ),
        Err(
            CoreStrongRequirementValidationError::UnusedExternalCallableBridge {
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
        verify_core_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            core_owner_set(core_exact_type("Mismatch")),
        ),
        Err(CoreStrongRequirementValidationError::ProducerMismatch {
            object: producer,
            bridge: ConeIdentity::CORE,
        })
    );
}

fn core_callable_bridge(name: &str) -> StrongExternalLirBridgeV1 {
    let target = StrongCallableDefinitionOwner::Function(core_function(name));
    let unit = core_exact_type("Unit");
    StrongExternalLirBridgeV1::Callable(
        StrongExternalCallableBridgeV1::new(
            target,
            CanonicalScoopAbiFunctionSignature::new(
                ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
                Vec::new(),
                ScoopAbiReturn::unit_void(),
                GcEffect::Managed,
            )
            .unwrap(),
            CallingConvention::Cdecl,
            CoreExternalCallableRootPlan::ManagedStatepoint,
        )
        .unwrap(),
    )
}

#[test]
fn rejects_missing_or_non_core_owner_authority() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "missingCoreOwner");
    let target = core_exact_type("Required");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(target).unwrap(),
    );
    let expected_name = bridge_name(&bridge);
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &source,
            &expected_name,
        )])
        .unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap();
    let expected_owner = StrongDefinitionOwnerV1::new(
        StrongDefinitionEntity::exact_type(target),
        StrongDefinitionRole::TypeDescriptor,
    )
    .unwrap();
    assert_eq!(
        verify_core_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            core_owner_set(core_exact_type("Different")),
        ),
        Err(CoreStrongRequirementValidationError::MissingCoreOwner {
            name: expected_name,
            owner: expected_owner,
        })
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
    let non_core_owners =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&non_core_strong)
            .unwrap();
    assert_eq!(
        verify_core_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            non_core_owners,
        ),
        Err(CoreStrongRequirementValidationError::CoreOwnerProducerMismatch { actual: producer })
    );
}

fn bridge_name(bridge: &StrongExternalLirBridgeV1) -> Vec<u8> {
    let request = match bridge {
        StrongExternalLirBridgeV1::Callable(bridge) => bridge.expected_symbol(),
        StrongExternalLirBridgeV1::TypeDescriptor(bridge) => bridge.expected_symbol(),
    };
    LirTargetProfile::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(request.symbol().as_str())
        .into_bytes()
}

fn type_registration_name(target: PersistentExactTypeId) -> Vec<u8> {
    let request = scoop_identity::PersistentSymbolRequest::new(
        scoop_identity::PersistentSymbolKey::TypeRegistration(target),
        scoop_identity::LinkageClass::ConeStrong,
    )
    .unwrap();
    LirTargetProfile::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(request.symbol().as_str())
        .into_bytes()
}

fn core_exact_type(name: &str) -> PersistentExactTypeId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
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

fn core_function(name: &str) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
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

fn core_owner_set(target: PersistentExactTypeId) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    let descriptor = fixture_for_type_descriptor(ConeIdentity::CORE, target);
    let registration = fixture_for_type_registration(ConeIdentity::CORE, target);
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&descriptor),
        verified_member_without_relocations(&registration),
    ])
    .unwrap();
    CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap()
}

fn descriptor_only_core_owner_set(
    target: PersistentExactTypeId,
) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    let descriptor = fixture_for_type_descriptor(ConeIdentity::CORE, target);
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&descriptor),
    ])
    .unwrap();
    CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap()
}

fn core_callable_owner_set(name: &str) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    let callable = fixture_for_producer(ConeIdentity::CORE, name);
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&callable),
    ])
    .unwrap();
    CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap()
}
