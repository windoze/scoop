mod fixture;
use super::*;
use crate::{
    verify_dependency_strong_requirements_v1, verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_v1,
};
use fixture::{SemanticFixture, semantic_fixture, semantic_fixture_with_additional_contracts};

#[test]
fn classifies_generated_c_target_support() {
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
        finish_builtin_external_requirements_v1(verified)
            .unwrap()
            .producer(),
        producer
    );
}

#[test]
fn bridge_recipe_distinguishes_a_helper_from_an_unrelated_source_name_collision() {
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
fn source_extern_target_wins_over_same_spelling_helper() {
    let fixture = semantic_fixture("memcpy", &[b"_memcpy"]);
    let actual = classify_c_bridge_target_support_requirements_v1(
        runtime_closure(&fixture),
        &fixture.bridge_plan,
        &fixture.profile,
    )
    .unwrap();
    assert_eq!(actual.source_external_requirements().len(), 1);
    assert!(actual.target_support_requirements().is_empty());
}

#[test]
fn scoop_codegen_helpers_use_the_target_registry() {
    let runtime = super::super::runtime_requirements::tests::classify(b"_memcpy");
    let profile = super::super::generated_c_objects::tests::profile(
        "clang-2100.1.1.101",
        0x000d_0100,
        0x000e_0200,
    );
    let plan = super::super::undefined_requirements::tests::empty_bridge_plan(runtime.producer());
    let actual =
        classify_c_bridge_target_support_requirements_v1(runtime, &plan, &profile).unwrap();
    assert_eq!(actual.target_support_requirements().len(), 1);
    assert!(actual.remaining_external_candidates().is_empty());
}

pub(in crate::link_object) fn support_closure(
    include_memcpy_source_contract: bool,
) -> BuiltinExternalRequirementsV1 {
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
    classify_c_bridge_target_support_requirements_v1(
        runtime_and_eh,
        &fixture.bridge_plan,
        &fixture.profile,
    )
    .unwrap()
}

fn runtime_closure(fixture: &SemanticFixture) -> VerifiedRuntimeAndEhRequirementClosureV1 {
    let strong = fixture
        .scoop_patch_sites
        .builtins()
        .strong_relocations()
        .clone();
    let core =
        verify_dependency_strong_requirements_v1(scoop_target_data::DARWIN_AARCH64, strong, &[])
            .unwrap();
    let source =
        verify_source_external_requirements_v1(core, fixture.native_requirements.clone()).unwrap();
    verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
        .unwrap()
}
