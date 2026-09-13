use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_lir::{
    LirTargetProfile, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
    StrongExternalTypeDescriptorBridgeV1,
};

use super::super::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use super::super::symbol_verification::tests::fixture_for_producer;
use super::*;
use crate::verify_current_cone_strong_relocation_closure_v1;

#[test]
fn resolves_core_bridges_and_preserves_other_external_candidates() {
    let producer = ConeIdentity::SINGLE_FILE;
    let source = fixture_for_producer(producer, "coreConsumer");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(core_exact_type("String")).unwrap(),
    );
    let expected_name = bridge_name(&bridge);
    let member = verified_member_with_undefined(&source, &expected_name);
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let surface =
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge.clone()]).unwrap();

    let verified =
        verify_core_strong_requirements_v1(LirTargetProfile::DARWIN_AARCH64, strong, surface)
            .unwrap();

    assert_eq!(verified.producer(), producer);
    assert_eq!(verified.core_requirements().len(), 1);
    assert_eq!(verified.core_requirements()[0].bridge(), &bridge);
    assert_eq!(
        verified.core_requirements()[0].use_site().symbol(),
        expected_name
    );
    assert!(verified.remaining_external_candidates().is_empty());

    let source = fixture_for_producer(producer, "nativeConsumer");
    let member = verified_member_with_undefined(&source, b"_native_symbol");
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![member]).unwrap();
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let verified =
        verify_core_strong_requirements_v1(LirTargetProfile::DARWIN_AARCH64, strong, surface)
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
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(core_exact_type("Unit")).unwrap(),
    );
    let expected_name = bridge_name(&bridge);
    let surface = StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap();
    assert_eq!(
        verify_core_strong_requirements_v1(LirTargetProfile::DARWIN_AARCH64, strong, surface),
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
        verify_core_strong_requirements_v1(LirTargetProfile::DARWIN_AARCH64, strong, surface),
        Err(CoreStrongRequirementValidationError::ProducerMismatch {
            object: producer,
            bridge: ConeIdentity::CORE,
        })
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
