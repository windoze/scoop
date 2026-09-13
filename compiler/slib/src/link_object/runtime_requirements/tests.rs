use scoop_identity::ConeIdentity;
use scoop_lir::{RuntimeAbiSymbolV1, TargetEhSupportV1, ValidatedLirTargetSelection};

use super::super::native_requirements::tests::{core_closure, native_surface};
use super::super::strong_relocation_closure::tests::verified_member_with_undefined;
use super::super::symbol_verification::tests::fixture_for_producer;
use super::*;
use crate::verify_source_external_requirements_v1;

#[test]
fn classifies_only_exact_runtime_contract_symbols() {
    let verified = classify(b"_scoop_rt_allocation_context");

    assert_eq!(verified.runtime_requirements().len(), 1);
    assert!(verified.target_eh_requirements().is_empty());
    assert!(verified.remaining_external_candidates().is_empty());
    let requirement = &verified.runtime_requirements()[0];
    assert_eq!(
        requirement.contract().symbol(),
        RuntimeAbiSymbolV1::AllocationContext
    );
    assert_eq!(
        requirement.use_site().symbol(),
        b"_scoop_rt_allocation_context"
    );

    let unregistered = classify(b"_scoop_rt_allocation_context_extra");
    assert!(unregistered.runtime_requirements().is_empty());
    assert_eq!(unregistered.remaining_external_candidates().len(), 1);
}

#[test]
fn classifies_only_the_closed_target_eh_symbols() {
    let personality = classify(b"_scoop_eh_personality");
    assert!(personality.runtime_requirements().is_empty());
    assert_eq!(personality.target_eh_requirements().len(), 1);
    assert_eq!(
        personality.target_eh_requirements()[0]
            .requirement()
            .support(),
        TargetEhSupportV1::ScoopPersonality
    );

    let unwind = classify(b"__Unwind_Resume");
    assert_eq!(
        unwind.target_eh_requirements()[0].requirement().support(),
        TargetEhSupportV1::UnwindResume
    );

    let non_eh_support = classify(b"_memcpy");
    assert!(non_eh_support.target_eh_requirements().is_empty());
    assert_eq!(non_eh_support.remaining_external_candidates().len(), 1);
}

#[test]
fn scoop_lir_closure_rejects_every_unclassified_external() {
    let runtime = classify(b"_scoop_rt_gc_stats");
    let producer = runtime.producer();
    let sealed = seal_scoop_lir_external_requirements_v1(runtime).unwrap();
    assert_eq!(sealed.producer(), producer);

    let unknown = classify(b"_memcpy");
    let binding = &unknown.remaining_external_candidates()[0];
    let member = binding.source_member();
    let atom = binding.containing_atom();
    assert_eq!(
        seal_scoop_lir_external_requirements_v1(unknown),
        Err(
            ScoopLirExternalRequirementClosureError::UnclassifiedExternalRelocation {
                member,
                atom,
                symbol: b"_memcpy".to_vec(),
            }
        )
    );
}

pub(in crate::link_object) fn classify(symbol: &[u8]) -> VerifiedRuntimeAndEhRequirementClosureV1 {
    let producer = ConeIdentity::SINGLE_FILE;
    let object = fixture_for_producer(producer, "runtimeRequirementConsumer");
    let member = verified_member_with_undefined(&object, symbol);
    let core = core_closure(producer, member);
    let native = native_surface(producer, Vec::new(), Vec::new());
    let source = verify_source_external_requirements_v1(core, native).unwrap();
    verify_runtime_and_eh_requirements_v1(
        source,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap()
}
