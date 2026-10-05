use std::collections::BTreeMap;

use scoop_identity::{
    CDataPointee, CPointerStorage, CallableBodyKey, CallableMaterialization,
    CallableMaterializationContext, CallableTemplateOwner, CallbackParameterIndex,
    CanonicalCAbiFunctionSignature, CanonicalCAbiParameter, CanonicalCAbiReturn,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType, CanonicalIdentifier,
    ConeIdentity, DeclarationScope, DefinitionAtomRole, DefinitionOwnerChain, DigestNodeId,
    DigestNodeKey, DigestPatchIntentKey, DigestSemanticFieldRole, Effect, ExactCallableSignature,
    ExactTypeKey, GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitId,
    GeneratedBridgeUnitKey, GeneratedCallableKey, NativeExternalContract, NativeLibraryBinding,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    StaticNoGcCallbackStorageBridgeId, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::{
    CBridgeProductionSetV1, DigestFinalizationPlanV1, DigestNodeV1, GeneratedBridgePlanSetV1,
    LirTargetProfile, ObjectSymbolSurfaceV1,
};

use super::*;
use crate::{
    GeneratedCBridgeObjectCandidateV1, PlannedStrongObjectSymbolRoleV1,
    PlannedStrongObjectSymbolSetV1, ProvisionalDigestPatchSiteV1, ScoopLirObjectCandidateV1,
    StrongRelocationResolutionV1, VerifiedObjectRelocationFormV1,
    verify_builtin_object_strong_relocations_v1, verify_c_bridge_production_envelopes_v1,
    verify_scoop_lir_digest_patch_sites_v1,
};

use super::super::c_bridge_production::tests::{fixture, member_plan, profile};
use super::super::native_requirements::tests::{contract_record, native_surface};
use super::super::relocation_verification::tests::add_undefined_symbols;
use super::super::strong_relocation_closure::tests::synthetic_binding;
use super::super::symbol_verification::tests::fixture_named;
use super::super::symbol_verification::tests::object_for_plan_with_deployment;

const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;

#[test]
fn verifies_outbound_function_definition_and_native_relocation() {
    let fixture = semantic_fixture("native_bridge", &[b"_native_bridge"]);
    let verified = verify_generated_c_bridge_semantics_v1(
        fixture.scoop_patch_sites,
        fixture.bridge_plan,
        fixture.native_requirements,
        &fixture.profile,
    )
    .unwrap();

    assert_eq!(verified.producer(), ConeIdentity::CORE);
    assert_eq!(verified.target(), LirTargetProfile::DARWIN_AARCH64);
    assert_eq!(verified.uses().len(), 1);
    assert!(matches!(
        verified.uses()[0].semantic(),
        GeneratedBridgeRelocationSemanticV1::NativeExternal { .. }
    ));
    assert_eq!(
        verified.uses()[0].unit(),
        verified.bridge_plan().units()[0].unit()
    );
}

#[test]
fn rejects_an_unplanned_external_and_a_missing_semantic_use() {
    let wrong = semantic_fixture("native_bridge", &[b"_other_native"]);
    assert!(matches!(
        verify_generated_c_bridge_semantics_v1(
            wrong.scoop_patch_sites,
            wrong.bridge_plan,
            wrong.native_requirements,
            &wrong.profile,
        ),
        Err(GeneratedCBridgeSemanticValidationError::UnexpectedPrimaryRelocation {
            symbol,
            ..
        }) if symbol == b"_other_native"
    ));

    let missing = semantic_fixture("native_bridge", &[]);
    assert!(matches!(
        verify_generated_c_bridge_semantics_v1(
            missing.scoop_patch_sites,
            missing.bridge_plan,
            missing.native_requirements,
            &missing.profile,
        ),
        Err(GeneratedCBridgeSemanticValidationError::MissingRequiredRelocation(_))
    ));
}

#[test]
fn target_support_cannot_replace_the_required_native_use() {
    let support_only = semantic_fixture("native_bridge", &[b"_memcpy"]);
    assert!(matches!(
        verify_generated_c_bridge_semantics_v1(
            support_only.scoop_patch_sites,
            support_only.bridge_plan,
            support_only.native_requirements,
            &support_only.profile,
        ),
        Err(GeneratedCBridgeSemanticValidationError::MissingRequiredRelocation(_))
    ));
}

#[test]
fn source_extern_target_wins_over_same_spelling_target_support() {
    let fixture = semantic_fixture("memcpy", &[b"_memcpy"]);
    let verified = verify_generated_c_bridge_semantics_v1(
        fixture.scoop_patch_sites,
        fixture.bridge_plan,
        fixture.native_requirements,
        &fixture.profile,
    )
    .unwrap();
    assert!(matches!(
        verified.uses()[0].semantic(),
        GeneratedBridgeRelocationSemanticV1::NativeExternal { .. }
    ));
}

#[test]
fn validates_each_global_bridge_against_its_data_mutability_contract() {
    let contract = contract_record(
        ConeIdentity::CORE,
        "globalBridgeDeclaration",
        "global_bridge",
        NativeLibraryBinding::DefaultNativeNamespace,
    )
    .fingerprint();
    let storage = CanonicalCStorageType::Boolean {
        exact_type: unit_exact_type(),
    };
    let read_only_data = NativeExternalContract::read_only_data(
        NativeLibraryBinding::DefaultNativeNamespace,
        storage,
    );
    let mutable_data =
        NativeExternalContract::mutable_data(NativeLibraryBinding::DefaultNativeNamespace, storage);
    let read_only_tls = NativeExternalContract::read_only_tls(
        NativeLibraryBinding::DefaultNativeNamespace,
        storage,
    );
    let mutable_tls =
        NativeExternalContract::mutable_tls(NativeLibraryBinding::DefaultNativeNamespace, storage);

    for key in [
        GeneratedBridgeUnitKey::GlobalRead(contract),
        GeneratedBridgeUnitKey::GlobalAddress(contract),
    ] {
        assert!(
            validate_native_contract_kind(
                GeneratedBridgeUnitId::from_key(&key).unwrap(),
                key,
                &read_only_data,
            )
            .is_ok()
        );
        assert!(
            validate_native_contract_kind(
                GeneratedBridgeUnitId::from_key(&key).unwrap(),
                key,
                &read_only_tls,
            )
            .is_ok()
        );
    }
    let write = GeneratedBridgeUnitKey::GlobalWrite(contract);
    let write_id = GeneratedBridgeUnitId::from_key(&write).unwrap();
    assert!(matches!(
        validate_native_contract_kind(write_id, write, &read_only_data),
        Err(GeneratedCBridgeSemanticValidationError::NativeContractKindMismatch(id))
            if id == write_id
    ));
    assert!(matches!(
        validate_native_contract_kind(write_id, write, &read_only_tls),
        Err(GeneratedCBridgeSemanticValidationError::NativeContractKindMismatch(id))
            if id == write_id
    ));
    assert!(validate_native_contract_kind(write_id, write, &mutable_data).is_ok());
    assert!(validate_native_contract_kind(write_id, write, &mutable_tls).is_ok());
    assert!(native_relocation_form_matches(
        LirTargetProfile::DARWIN_AARCH64,
        write,
        &mutable_data,
        VerifiedObjectRelocationFormV1::Unsigned64,
    ));
    assert!(!native_relocation_form_matches(
        LirTargetProfile::DARWIN_AARCH64,
        write,
        &mutable_data,
        VerifiedObjectRelocationFormV1::TlvpLoadPage21,
    ));
    assert!(native_relocation_form_matches(
        LirTargetProfile::DARWIN_AARCH64,
        write,
        &mutable_tls,
        VerifiedObjectRelocationFormV1::TlvpLoadPage21,
    ));
    assert!(!native_relocation_form_matches(
        LirTargetProfile::DARWIN_AARCH64,
        write,
        &mutable_tls,
        VerifiedObjectRelocationFormV1::Unsigned64,
    ));
    assert!(!native_relocation_form_matches(
        LirTargetProfile::DARWIN_AARCH64,
        write,
        &mutable_tls,
        VerifiedObjectRelocationFormV1::Branch26,
    ));
}

#[test]
fn rejects_a_profile_other_than_the_one_bound_by_production() {
    let fixture = semantic_fixture("native_bridge", &[b"_native_bridge"]);
    let other_profile = profile("clang-2100.1.1.102", MINIMUM_OS, SDK);
    assert!(matches!(
        verify_generated_c_bridge_semantics_v1(
            fixture.scoop_patch_sites,
            fixture.bridge_plan,
            fixture.native_requirements,
            &other_profile,
        ),
        Err(GeneratedCBridgeSemanticValidationError::ProductionMismatch)
    ));
}

#[test]
fn classifies_managed_callback_gateway_and_signature_descriptor_separately() {
    let signature = signature();
    let unit_key = GeneratedBridgeUnitKey::CallbackTrampoline {
        signature,
        context_index: CallbackParameterIndex::new(0),
    };
    let unit_id = GeneratedBridgeUnitId::from_key(&unit_key).unwrap();
    let descriptor = scoop_identity::CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
        ConeIdentity::CORE,
        GeneratedBridgeAtomRoleKey::SignatureDescriptor {
            unit: unit_id,
            signature,
        },
    ))
    .unwrap();
    let descriptor_definition = derive_bridge_definition(&descriptor).unwrap().0;
    let source = fixture_named("managedCallbackSemanticSource");
    let unit = ExpectedBridgeUnit {
        unit: unit_id,
        producer: ConeIdentity::CORE,
        key: unit_key,
        signature_descriptor: Some((descriptor.id(), descriptor_definition)),
    };
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let support =
        CBridgeTargetSupportRegistryV1::current(LirTargetProfile::DARWIN_AARCH64, &profile)
            .unwrap();
    let runtime = RuntimeSymbolContractV1::current(
        LirTargetProfile::DARWIN_AARCH64,
        RuntimeAbiSymbolV1::CallbackInvoke,
    )
    .unwrap();
    let runtime_binding = synthetic_binding(
        source.symbols.member(),
        source.atom,
        VerifiedObjectRelocationFormV1::Branch26,
        &runtime.object_symbol(LirTargetProfile::DARWIN_AARCH64),
        StrongRelocationResolutionV1::ExternalCandidate {
            object_symbol_table_index: 3,
        },
    );
    let descriptor_binding = synthetic_binding(
        source.symbols.member(),
        source.atom,
        VerifiedObjectRelocationFormV1::Page21 {
            explicit_addend: None,
        },
        b"_descriptor",
        StrongRelocationResolutionV1::ObjectLocalStrong {
            target_member: source.symbols.member(),
            definition: descriptor_definition,
            owner: super::super::LinkDefinitionOwnerV1::GeneratedBridge(descriptor.id()),
        },
    );
    let native = BTreeMap::new();
    let runtime_semantic =
        classify_binding(&runtime_binding, &unit, &native, &runtime, &support).unwrap();
    let descriptor_semantic =
        classify_binding(&descriptor_binding, &unit, &native, &runtime, &support).unwrap();
    assert!(matches!(
        runtime_semantic,
        GeneratedBridgeRelocationSemanticV1::RuntimeCallbackInvoke { .. }
    ));
    assert_eq!(
        descriptor_semantic,
        GeneratedBridgeRelocationSemanticV1::SignatureDescriptor {
            atom: descriptor.id()
        }
    );
    let mut observed = ObservedUnitSemantics::default();
    observed.observe(runtime_semantic);
    observed.observe(descriptor_semantic);
    observed.finish(&unit).unwrap();
}

#[test]
fn classifies_static_callback_only_to_its_typed_storage_bridge() {
    let signature = signature();
    let storage_bridge = static_storage_bridge();
    let unit_key = GeneratedBridgeUnitKey::StaticCallbackTrampoline {
        storage_bridge,
        signature,
    };
    let unit_id = GeneratedBridgeUnitId::from_key(&unit_key).unwrap();
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::GeneratedCallable(storage_bridge.generated_callable()),
    ))
    .unwrap();
    let definition = ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let source = fixture_named("staticCallbackSemanticSource");
    let unit = ExpectedBridgeUnit {
        unit: unit_id,
        producer: ConeIdentity::CORE,
        key: unit_key,
        signature_descriptor: None,
    };
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let support =
        CBridgeTargetSupportRegistryV1::current(LirTargetProfile::DARWIN_AARCH64, &profile)
            .unwrap();
    let runtime = RuntimeSymbolContractV1::current(
        LirTargetProfile::DARWIN_AARCH64,
        RuntimeAbiSymbolV1::CallbackInvoke,
    )
    .unwrap();
    let binding = synthetic_binding(
        source.symbols.member(),
        source.atom,
        VerifiedObjectRelocationFormV1::Branch26,
        b"_storage_bridge",
        StrongRelocationResolutionV1::ObjectLocalStrong {
            target_member: source.symbols.member(),
            definition,
            owner: super::super::LinkDefinitionOwnerV1::GeneratedBridge(
                scoop_identity::CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(
                    ConeIdentity::CORE,
                    GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: unit_id },
                ))
                .unwrap()
                .id(),
            ),
        },
    );
    let semantic = classify_binding(&binding, &unit, &BTreeMap::new(), &runtime, &support).unwrap();
    assert_eq!(
        semantic,
        GeneratedBridgeRelocationSemanticV1::StaticCallbackStorageBridge { body }
    );
    let mut observed = ObservedUnitSemantics::default();
    observed.observe(semantic);
    observed.finish(&unit).unwrap();
}

pub(in crate::link_object) struct SemanticFixture {
    pub(in crate::link_object) scoop_patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    pub(in crate::link_object) bridge_plan: GeneratedBridgePlanSetV1,
    pub(in crate::link_object) native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    pub(in crate::link_object) profile: CBridgeToolchainProfileV1,
}

pub(in crate::link_object) fn semantic_fixture(
    native_name: &str,
    relocation_symbols: &[&[u8]],
) -> SemanticFixture {
    semantic_fixture_with_additional_contracts(native_name, &[], relocation_symbols)
}

pub(in crate::link_object) fn semantic_fixture_with_additional_contracts(
    native_name: &str,
    additional_native_names: &[&str],
    relocation_symbols: &[&[u8]],
) -> SemanticFixture {
    let fixture = fixture(Some(native_name));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let surface = ObjectSymbolSurfaceV1::from_foundation(&fixture.foundation).unwrap();
    let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
        LirTargetProfile::DARWIN_AARCH64,
        &surface,
        &member_plan,
    )
    .unwrap();
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);

    let scoop_member = member_plan.scoop_lir_members()[0].member_id();
    let scoop_object = object_for_plan_with_deployment(
        symbol_plan.member(scoop_member).unwrap(),
        |role| match role {
            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
            PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 64,
        },
        &[],
        None,
        &[0; 64],
    );
    let bridge_member = member_plan.generated_bridge_members()[0].member_id();
    let bridge_symbols = symbol_plan.member(bridge_member).unwrap();
    let primary = primary_role(bridge_symbols);
    let relocations = relocation_symbols
        .iter()
        .enumerate()
        .map(|(index, _)| (u32::try_from(index).unwrap() * 4, primary))
        .collect::<Vec<_>>();
    let mut bridge_bytes = object_for_plan_with_deployment(
        bridge_symbols,
        canonical_value,
        &relocations,
        Some((MINIMUM_OS, SDK, &[])),
        &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0],
    )
    .bytes;
    if !relocation_symbols.is_empty() {
        bridge_bytes = add_undefined_symbols(bridge_bytes, true, relocation_symbols);
    }
    let scoop_objects = [ScoopLirObjectCandidateV1::new(
        scoop_member,
        &scoop_object.bytes,
    )];
    let bridge_objects = [GeneratedCBridgeObjectCandidateV1::new(
        bridge_member,
        &bridge_bytes,
    )];
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let production_proof = verify_c_bridge_production_envelopes_v1(
        bridge_plan.clone(),
        production,
        &profile,
        &member_plan,
        &bridge_objects,
    )
    .unwrap();
    let builtins = verify_builtin_object_strong_relocations_v1(
        &member_plan,
        &symbol_plan,
        &scoop_objects,
        production_proof,
        &bridge_objects,
    )
    .unwrap();
    let image_key = DigestNodeKey::runtime_image(fixture.foundation.producer());
    let image_node_id = DigestNodeId::from_key(&image_key).unwrap();
    let image_patch = DigestPatchIntentKey::new(
        image_node_id,
        fixture.lir_plan,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let image_node = DigestNodeV1::new(image_key, Vec::new(), vec![image_patch]).unwrap();
    let image_intent = image_node.patch_intents()[0].id();
    let digest_plan = DigestFinalizationPlanV1::new(vec![image_node], &fixture.foundation).unwrap();
    let patch_sites = [ProvisionalDigestPatchSiteV1::new(
        image_intent,
        scoop_member,
        u64::try_from(scoop_object.section_offset).unwrap() + 16,
        32,
    )];
    let scoop_patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        builtins,
        &fixture.foundation,
        digest_plan,
        &scoop_objects,
        &patch_sites,
    )
    .unwrap();
    let mut contracts = vec![contract_record(
        ConeIdentity::CORE,
        "nativeBridgeDeclaration",
        native_name,
        NativeLibraryBinding::DefaultNativeNamespace,
    )];
    contracts.extend(
        additional_native_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                contract_record(
                    ConeIdentity::CORE,
                    &format!("additionalNativeBridgeDeclaration{index}"),
                    name,
                    NativeLibraryBinding::DefaultNativeNamespace,
                )
            }),
    );
    let native_requirements = native_surface(ConeIdentity::CORE, contracts, Vec::new());
    SemanticFixture {
        scoop_patch_sites,
        bridge_plan,
        native_requirements,
        profile,
    }
}

fn primary_role(
    symbols: &crate::PlannedMemberStrongObjectSymbolsV1,
) -> PlannedStrongObjectSymbolRoleV1 {
    symbols
        .symbols()
        .iter()
        .find_map(|symbol| match symbol.role() {
            role @ PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. } => Some(role),
            _ => None,
        })
        .unwrap()
}

fn canonical_value(role: PlannedStrongObjectSymbolRoleV1) -> u64 {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 8,
    }
}

fn signature() -> scoop_identity::CanonicalCAbiSignatureFingerprint {
    let context = unit_exact_type();
    CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
        vec![
            CanonicalCAbiParameter::new(
                context,
                CanonicalCStorageType::DataPointer {
                    exact_type: context,
                    pointee: CDataPointee::OpaqueUnit,
                    storage: CPointerStorage::Direct,
                },
            )
            .unwrap(),
        ],
        CanonicalCAbiReturn::Void,
    ))
    .unwrap()
    .fingerprint()
}

fn static_storage_bridge() -> StaticNoGcCallbackStorageBridgeId {
    let unit = unit_exact_type();
    StaticNoGcCallbackStorageBridgeId::from_key(
        &GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
            source: CallableMaterialization::new(
                CallableTemplateOwner::Function(source_function()),
                CallableMaterializationContext::NoSubstitution,
            ),
            signature: ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
        },
    )
    .unwrap()
}

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap()
}

fn source_function() -> PersistentFunctionId {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("staticCallbackSource").unwrap(),
        0,
        None,
        Vec::new(),
    );
    PersistentFunctionId::from_source_declaration(&declaration).unwrap()
}
