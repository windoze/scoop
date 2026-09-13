use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    LirTargetProfile, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
    StrongExternalTypeDescriptorBridgeV1,
};

use super::super::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use super::super::symbol_verification::tests::{fixture_for_producer, fixture_for_type_descriptor};
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
fn rejects_unused_bridges_and_cross_producer_surfaces() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "unusedBridge");
    let member = verified_member_without_relocations(&source);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let target = core_exact_type("Unit");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(target).unwrap(),
    );
    let expected_name = bridge_name(&bridge);
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap();
    assert_eq!(
        verify_core_strong_requirements_v1(
            LirTargetProfile::DARWIN_AARCH64,
            strong,
            surface,
            core_owner_set(target),
        ),
        Err(CoreStrongRequirementValidationError::UnusedExternalBridge {
            name: expected_name,
        })
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

fn core_owner_set(target: PersistentExactTypeId) -> CanonicalDefinedLinkSymbolOwnerSetV1 {
    let fixture = fixture_for_type_descriptor(ConeIdentity::CORE, target);
    let closure = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&fixture),
    ])
    .unwrap();
    CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap()
}
