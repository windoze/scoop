use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    NativeLibraryBinding, PackagePath, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_lir::{
    CBridgeTargetSupportRequirementV1, CBridgeTargetSupportV1, CanonicalLirFoundation,
    GeneratedBridgePlanSetV1, LirTargetProfile, OdrFreeLirFoundation,
    StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
    StrongExternalTypeDescriptorBridgeV1, ValidatedLirTargetSelection,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::super::c_bridge_production::tests::profile;
use super::super::c_bridge_target_requirements::{
    tests::support_closure, without_generated_bridge_semantics_for_test,
};
use super::super::current_cone_requirements::tests::primary_bridge_requirement;
use super::super::native_requirements::tests::{contract_record, native_surface};
use super::super::runtime_requirements::tests::classify;
use super::super::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use super::super::symbol_verification::tests::{
    fixture_for_producer, fixture_for_type_descriptor, fixture_for_type_registration, fixture_named,
};
use super::*;
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, seal_builtin_object_external_requirements_v1,
    verify_core_strong_requirements_v1, verify_current_cone_strong_relocation_closure_v1,
    verify_current_cone_undefined_requirements_v1, verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_v1,
};

#[test]
fn finalizes_runtime_and_eh_requirements_with_a_fixed_wire_vector() {
    let runtime = final_from_external(classify(b"_scoop_rt_allocation_context"));
    assert_projection_round_trip(&runtime);
    assert_eq!(runtime.requirements().len(), 1);
    assert!(matches!(
        runtime.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::RuntimeAbi { .. }
    ));
    assert_eq!(
        hex(&encode(&runtime).unwrap()),
        "81a201aa0158204944a769dc38fa9fb4ba158a185c5343204e5dbb92e8dabfc75dc31b3359be4f0258201ac89db56dadcbb38d882cd5fc8b7b9a8b894ee48d502b5afb345a59078b549c030104010500060407a10003081addccbbaa09010a581c5f73636f6f705f72745f616c6c6f636174696f6e5f636f6e7465787402a200050158204d8d73e6d4d595109a62671e6c0e6433eb90a696d799001db658399c5a20e1a4"
    );

    let eh = final_from_external(classify(b"__Unwind_Resume"));
    assert_projection_round_trip(&eh);
    assert!(matches!(
        eh.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::TargetEhSupport { .. }
    ));
}

#[test]
fn c_bridge_target_support_uses_the_seventh_requirement_tag() {
    let profile = profile("clang-2100.1.1.101", 0x000d_0100, 0x000e_0200);
    let contract = CBridgeTargetSupportRequirementV1::current(
        LirTargetProfile::DARWIN_AARCH64,
        &profile,
        CBridgeTargetSupportV1::Memcpy,
    )
    .unwrap()
    .id();
    let mut expected = vec![0xa2, 0x00, 0x07, 0x01];
    expected.extend(encode(&contract).unwrap());
    assert_eq!(
        encode(&FinalUndefinedSymbolRequirementV1::CBridgeTargetSupport { contract }).unwrap(),
        expected
    );
}

#[test]
fn finalizes_a_semantically_verified_c_bridge_target_support_use() {
    let external = support_closure(false);
    let strong = external.strong_closure().clone();
    let current = verify_current_cone_undefined_requirements_v1(
        strong,
        empty_bridge_plan(ConeIdentity::CORE),
    )
    .unwrap();
    let final_set = finalize_undefined_symbol_requirements_v1(
        current,
        seal_builtin_object_external_requirements_v1(external).unwrap(),
    )
    .unwrap();
    assert_projection_round_trip(&final_set);

    assert_eq!(final_set.requirements().len(), 2);
    assert!(final_set.requirements().iter().any(|requirement| matches!(
        requirement.requirement(),
        FinalUndefinedSymbolRequirementV1::CBridgeTargetSupport { .. }
    )));
}

#[test]
fn finalizes_current_cone_and_generated_bridge_requirements() {
    let source = fixture_named("finalIntraSource");
    let target = fixture_named("finalIntraTarget");
    let target_symbol = target
        .symbols
        .symbols()
        .iter()
        .find(|symbol| {
            matches!(
                symbol.role(),
                crate::PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
            )
        })
        .unwrap();
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_undefined(&source, target_symbol.macho_name()),
        verified_member_without_relocations(&target),
    ])
    .unwrap();
    let current = verify_current_cone_undefined_requirements_v1(
        strong.clone(),
        empty_bridge_plan(ConeIdentity::CORE),
    )
    .unwrap();
    let final_set =
        finalize_undefined_symbol_requirements_v1(current, sealed_without_externals(strong))
            .unwrap();
    assert_projection_round_trip(&final_set);
    assert!(matches!(
        final_set.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::IntraConeStrong { .. }
    ));

    let (current, unit) = primary_bridge_requirement();
    let strong = current.strong_closure().clone();
    let final_set =
        finalize_undefined_symbol_requirements_v1(current, sealed_without_externals(strong))
            .unwrap();
    assert_projection_round_trip(&final_set);
    assert_eq!(
        final_set.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::GeneratedBridge { unit }
    );
}

#[test]
fn finalizes_core_and_source_external_requirements() {
    let producer = ConeIdentity::SINGLE_FILE;
    let core_target = core_exact_type("FinalCoreType");
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(core_target).unwrap(),
    );
    let bridge_name = normalized_bridge_name(&bridge);
    let source = fixture_for_producer(producer, "finalCoreConsumer");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &source,
            &bridge_name,
        )])
        .unwrap();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(producer))
            .unwrap();
    let bridges = StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap();
    let core = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        bridges,
        core_owner_set(core_target),
    )
    .unwrap();
    let source = verify_source_external_requirements_v1(
        core,
        native_surface(producer, Vec::new(), Vec::new()),
    )
    .unwrap();
    let external = verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let final_set =
        finalize_undefined_symbol_requirements_v1(current, seal_without_generated(external))
            .unwrap();
    assert_projection_round_trip(&final_set);
    assert!(matches!(
        final_set.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::CoreStrong {
            core: ConeIdentity::CORE,
            ..
        }
    ));

    let source = fixture_for_producer(producer, "finalNativeConsumer");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &source,
            b"_final_native",
        )])
        .unwrap();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(producer))
            .unwrap();
    let core = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
        core_owner_set(core_exact_type("UnrelatedCoreType")),
    )
    .unwrap();
    let native = native_surface(
        producer,
        vec![contract_record(
            producer,
            "finalNativeDeclaration",
            "final_native",
            NativeLibraryBinding::DefaultNativeNamespace,
        )],
        Vec::new(),
    );
    let source = verify_source_external_requirements_v1(core, native).unwrap();
    let external = verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let final_set =
        finalize_undefined_symbol_requirements_v1(current, seal_without_generated(external))
            .unwrap();
    assert_projection_round_trip(&final_set);
    assert!(matches!(
        final_set.requirements()[0].requirement(),
        FinalUndefinedSymbolRequirementV1::SourceExtern {
            library: NativeLibraryBinding::DefaultNativeNamespace,
            ..
        }
    ));
}

#[test]
fn rejects_proofs_built_from_different_strong_closures() {
    let source = fixture_named("mismatchedFinalClosure");
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&source),
    ])
    .unwrap();
    let current = verify_current_cone_undefined_requirements_v1(
        strong,
        empty_bridge_plan(ConeIdentity::CORE),
    )
    .unwrap();
    let external = seal_without_generated(classify(b"_scoop_rt_allocation_context"));

    assert_eq!(
        finalize_undefined_symbol_requirements_v1(current, external),
        Err(UndefinedSymbolRequirementFinalizationError::StrongClosureMismatch)
    );
}

#[test]
fn decoded_requirement_projection_rejects_another_verified_object_projection() {
    let runtime = final_from_external(classify(b"_scoop_rt_allocation_context"));
    let eh = final_from_external(classify(b"__Unwind_Resume"));
    let decoded = decode_canonical::<DecodedCanonicalUndefinedSymbolRequirementSetV1>(
        &encode(&runtime).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        decoded.validate_against(&eh),
        Err(UndefinedSymbolRequirementValidationError::ProjectionMismatch)
    ));
}

#[test]
fn decoded_requirement_sum_rejects_unknown_and_incomplete_variants() {
    for bytes in [
        vec![0xa1, 0x00, 0x18, 0xff],
        vec![0xa1, 0x00, 0x01],
        vec![0xa2, 0x00, 0x05, 0x01, 0x58, 0x1f],
    ] {
        assert!(
            decode_canonical::<DecodedFinalUndefinedSymbolRequirementV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
}

fn assert_projection_round_trip(expected: &CanonicalUndefinedSymbolRequirementSetV1) {
    let bytes = encode(expected).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalUndefinedSymbolRequirementSetV1>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    let actual = decoded.validate_against(expected).unwrap();
    assert_eq!(actual, *expected);
}

fn final_from_external(
    external: crate::VerifiedRuntimeAndEhRequirementClosureV1,
) -> CanonicalUndefinedSymbolRequirementSetV1 {
    let producer = external.producer();
    let strong = external
        .source_closure()
        .core_closure()
        .strong_closure()
        .clone();
    let current =
        verify_current_cone_undefined_requirements_v1(strong, empty_bridge_plan(producer)).unwrap();
    finalize_undefined_symbol_requirements_v1(current, seal_without_generated(external)).unwrap()
}

pub(in crate::link_object) fn sealed_without_externals(
    strong: crate::VerifiedCurrentConeStrongRelocationClosureV1,
) -> SealedBuiltinObjectExternalRequirementClosureV1 {
    let producer = strong.producer();
    let core_owners =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong).unwrap();
    let core = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
        core_owners,
    )
    .unwrap();
    let source = verify_source_external_requirements_v1(
        core,
        native_surface(producer, Vec::new(), Vec::new()),
    )
    .unwrap();
    let external = verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    seal_without_generated(external)
}

pub(in crate::link_object) fn empty_final_requirements_for_strong(
    strong: crate::VerifiedCurrentConeStrongRelocationClosureV1,
) -> CanonicalUndefinedSymbolRequirementSetV1 {
    let producer = strong.producer();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(producer))
            .unwrap();
    let core = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
        core_owner_set(core_exact_type("CallableFingerprintCoreAuthority")),
    )
    .unwrap();
    let source = verify_source_external_requirements_v1(
        core,
        native_surface(producer, Vec::new(), Vec::new()),
    )
    .unwrap();
    let external = verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    finalize_undefined_symbol_requirements_v1(current, seal_without_generated(external)).unwrap()
}

pub(in crate::link_object) fn core_type_final_requirements_for_strong(
    strong: crate::VerifiedCurrentConeStrongRelocationClosureV1,
    target: PersistentExactTypeId,
) -> CanonicalUndefinedSymbolRequirementSetV1 {
    let producer = strong.producer();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(producer))
            .unwrap();
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(target).unwrap(),
    );
    let core = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![bridge]).unwrap(),
        core_owner_set(target),
    )
    .unwrap();
    let source = verify_source_external_requirements_v1(
        core,
        native_surface(producer, Vec::new(), Vec::new()),
    )
    .unwrap();
    let external = verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    finalize_undefined_symbol_requirements_v1(current, seal_without_generated(external)).unwrap()
}

fn seal_without_generated(
    external: crate::VerifiedRuntimeAndEhRequirementClosureV1,
) -> SealedBuiltinObjectExternalRequirementClosureV1 {
    seal_builtin_object_external_requirements_v1(without_generated_bridge_semantics_for_test(
        external,
    ))
    .unwrap()
}

pub(in crate::link_object) fn empty_bridge_plan(
    producer: ConeIdentity,
) -> GeneratedBridgePlanSetV1 {
    let foundation =
        OdrFreeLirFoundation::try_new(producer, CanonicalLirFoundation::empty()).unwrap();
    GeneratedBridgePlanSetV1::from_odr_free_foundation(&foundation).unwrap()
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

fn normalized_bridge_name(bridge: &StrongExternalLirBridgeV1) -> Vec<u8> {
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
