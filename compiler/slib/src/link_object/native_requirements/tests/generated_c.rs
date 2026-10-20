//! Generated C uses the same source contracts and ordinary native requirements.

use crate::{
    FinalUndefinedSymbolRequirementV1, VerifiedRuntimeAndEhRequirementClosureV1,
    finalize_undefined_symbol_requirements_v1, verify_current_cone_undefined_requirements_v1,
    verify_dependency_strong_requirements_v1, verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_v1,
};

mod fixture;
use fixture::{SemanticFixture, semantic_fixture};

#[test]
fn generated_c_keeps_auxiliary_symbols_without_a_registry() {
    let fixture = semantic_fixture("native_bridge", &[b"_native_bridge", b"_unlisted_helper"]);
    let external = runtime_closure(&fixture);
    assert_eq!(
        external
            .source_closure()
            .source_external_requirements()
            .len(),
        1
    );
    assert_eq!(external.remaining_external_candidates().len(), 1);

    let current = verify_current_cone_undefined_requirements_v1(
        fixture.builtins.strong_relocations().clone(),
        fixture.bridge_plan,
    )
    .unwrap();
    let requirements = finalize_undefined_symbol_requirements_v1(current, external);
    assert_eq!(requirements.requirements().len(), 2);
    assert!(requirements.requirements().iter().any(|requirement| {
        requirement.requirement()
            == FinalUndefinedSymbolRequirementV1::Native {
                symbol: b"_unlisted_helper".to_vec(),
            }
    }));
}

#[test]
fn explicit_libc_source_declarations_keep_their_ffi_contract() {
    let fixture = semantic_fixture("memcpy", &[b"_memcpy"]);
    let external = runtime_closure(&fixture);
    assert_eq!(
        external
            .source_closure()
            .source_external_requirements()
            .len(),
        1
    );
    assert!(external.remaining_external_candidates().is_empty());
}

fn runtime_closure(fixture: &SemanticFixture) -> VerifiedRuntimeAndEhRequirementClosureV1 {
    let strong = fixture.builtins.strong_relocations().clone();
    let dependencies =
        verify_dependency_strong_requirements_v1(scoop_target_data::DARWIN_AARCH64, strong, &[])
            .unwrap();
    let source =
        verify_source_external_requirements_v1(dependencies, fixture.native_requirements.clone())
            .unwrap();
    verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
        .unwrap()
}
