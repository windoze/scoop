use scoop_identity::ConeIdentity;
use scoop_lir::{LirTargetProfile, StrongExternalLirBridgeSurfaceV1, ValidatedLirTargetSelection};

use super::super::generated_bridge_semantics::tests::{
    SemanticFixture, semantic_fixture, semantic_fixture_with_additional_contracts,
};
use super::super::runtime_requirements::tests::classify;
use super::*;
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, verify_core_strong_requirements_v1,
    verify_generated_c_bridge_semantics_v1, verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_v1,
};

#[test]
fn classifies_only_semantically_proven_generated_c_target_support() {
    let verified = support_closure(false);

    assert_eq!(verified.source_external_requirements().len(), 1);
    assert!(verified.runtime_requirements().is_empty());
    assert!(verified.target_eh_requirements().is_empty());
    assert_eq!(verified.target_support_requirements().len(), 1);
    assert!(verified.remaining_external_candidates().is_empty());
    let support = &verified.target_support_requirements()[0];
    assert_eq!(
        support.requirement().support(),
        scoop_lir::CBridgeTargetSupportV1::Memcpy
    );
    assert_eq!(support.use_site().symbol(), b"_memcpy");
    let producer = verified.producer();
    assert_eq!(
        seal_builtin_object_external_requirements_v1(verified)
            .unwrap()
            .producer(),
        producer
    );
}

#[test]
fn producer_semantics_reclassifies_a_helper_despite_an_unrelated_source_name_collision() {
    let verified = support_closure(true);

    assert_eq!(verified.source_external_requirements().len(), 1);
    assert_eq!(verified.target_support_requirements().len(), 1);
    assert_eq!(
        verified.source_external_requirements()[0]
            .use_site()
            .symbol(),
        b"_native_bridge"
    );
}

#[test]
fn rejects_requirement_and_semantic_proofs_from_different_strong_closures() {
    let fixture = semantic_fixture("native_bridge", &[b"_native_bridge"]);
    let semantic = verify_generated_c_bridge_semantics_v1(
        fixture.scoop_patch_sites,
        fixture.bridge_plan,
        fixture.native_requirements,
        &fixture.profile,
    )
    .unwrap();

    assert_eq!(
        verify_c_bridge_target_support_requirements_v1(classify(b"_memcpy"), semantic),
        Err(CBridgeTargetSupportRequirementValidationError::StrongClosureMismatch)
    );
}

#[test]
fn a_scoop_member_cannot_borrow_generated_c_target_support() {
    let unknown = without_generated_bridge_semantics_for_test(classify(b"_memcpy"));
    let binding = &unknown.remaining_external_candidates()[0];
    let member = binding.source_member();
    let atom = binding.containing_atom();

    assert_eq!(
        seal_builtin_object_external_requirements_v1(unknown),
        Err(
            BuiltinObjectExternalRequirementClosureError::UnclassifiedExternalRelocation {
                member,
                atom,
                symbol: b"_memcpy".to_vec(),
            }
        )
    );
}

pub(in crate::link_object) fn support_closure(
    include_memcpy_source_contract: bool,
) -> VerifiedCBridgeTargetSupportRequirementClosureV1 {
    let fixture = if include_memcpy_source_contract {
        semantic_fixture_with_additional_contracts(
            "native_bridge",
            &["memcpy"],
            &[b"_native_bridge", b"_memcpy"],
        )
    } else {
        semantic_fixture("native_bridge", &[b"_native_bridge", b"_memcpy"])
    };
    let runtime_and_eh = runtime_closure(&fixture);
    let bridge_semantics = verify_generated_c_bridge_semantics_v1(
        fixture.scoop_patch_sites,
        fixture.bridge_plan,
        fixture.native_requirements,
        &fixture.profile,
    )
    .unwrap();
    verify_c_bridge_target_support_requirements_v1(runtime_and_eh, bridge_semantics).unwrap()
}

fn runtime_closure(fixture: &SemanticFixture) -> VerifiedRuntimeAndEhRequirementClosureV1 {
    let strong = fixture
        .scoop_patch_sites
        .builtins()
        .strong_relocations()
        .clone();
    let owners =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong).unwrap();
    let core = verify_core_strong_requirements_v1(
        LirTargetProfile::DARWIN_AARCH64,
        strong,
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::CORE, Vec::new()).unwrap(),
        owners,
    )
    .unwrap();
    let source =
        verify_source_external_requirements_v1(core, fixture.native_requirements.clone()).unwrap();
    verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap()
}
