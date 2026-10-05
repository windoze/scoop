use super::*;
use crate::link_object::ObjectStackmapSectionError;

pub(crate) mod support;
use support::{Corruption, Fixture};

#[test]
fn closes_full_lir_member_symbol_record_and_machine_code_proofs() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified =
        verify_scoop_lir_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects).unwrap();

    assert_eq!(
        verified.producer(),
        scoop_identity::ConeIdentity::SINGLE_FILE
    );
    assert_eq!(verified.records().len(), 2);
    assert!(
        verified
            .records()
            .windows(2)
            .all(|pair| pair[0].normalized().canonical().site()
                < pair[1].normalized().canonical().site())
    );
    assert_eq!(
        verified
            .records()
            .iter()
            .map(|record| record.normalized().canonical().site())
            .collect::<Vec<_>>(),
        verified
            .semantic_plan()
            .sites()
            .iter()
            .map(|site| site.site())
            .collect::<Vec<_>>()
    );
    assert!(verified.records().iter().all(|record| {
        record.member() == fixture.member
            && record.normalized().canonical().root_pair_count() == 0
            && record.normalized().canonical().locations().len() == 3
    }));
    let mut return_pcs = verified
        .records()
        .iter()
        .map(VerifiedScoopLirStackmapRecordV1::object_return_pc)
        .collect::<Vec<_>>();
    return_pcs.sort_unstable();
    assert_eq!(return_pcs, [12, 16]);
}

#[test]
fn rejects_a_record_not_present_in_the_lir_semantic_proof() {
    let fixture = Fixture::new(Corruption::UnknownSafepoint);
    let member = fixture.member;
    let objects = [ScoopLirObjectCandidateV1::new(
        member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_scoop_lir_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects),
        Err(ScoopLirStackmapValidationError::UnexpectedSafepointId {
            member: actual,
            ..
        }) if actual == member
    ));
}

#[test]
fn rejects_a_non_stackmap_atom_owning_the_physical_stackmap_section() {
    let fixture = Fixture::new(Corruption::WrongStackmapAtomRole);
    let member = fixture.member;
    let objects = [ScoopLirObjectCandidateV1::new(
        member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_scoop_lir_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects),
        Err(ScoopLirStackmapValidationError::NonStackmapAtomInSection {
            member: actual,
            actual: scoop_identity::DefinitionAtomRole::AddressTakenConstant,
            ..
        }) if actual == member
    ));
}

#[test]
fn rejects_return_pcs_that_do_not_immediately_follow_aarch64_calls() {
    let fixture = Fixture::new(Corruption::NonCallReturnPc);
    let member = fixture.member;
    let objects = [ScoopLirObjectCandidateV1::new(
        member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_scoop_lir_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects),
        Err(ScoopLirStackmapValidationError::MachineCode {
            member: actual,
            source: StackmapMachineCodeError::Aarch64(DarwinAarch64StackmapMachineCodeError::ReturnPcDoesNotFollowCall { .. }),
            ..
        }) if actual == member
    ));
}

#[test]
fn rejects_managed_functions_without_an_aarch64_frame_chain() {
    let fixture = Fixture::new(Corruption::MissingFrameChain);
    let member = fixture.member;
    let objects = [ScoopLirObjectCandidateV1::new(
        member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_scoop_lir_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects),
        Err(ScoopLirStackmapValidationError::MachineCode {
            member: actual,
            source: StackmapMachineCodeError::Aarch64(DarwinAarch64StackmapMachineCodeError::MissingManagedFrameChain { .. }),
            ..
        }) if actual == member
    ));
}

#[test]
fn rejects_object_bytes_changed_after_the_builtin_object_proof() {
    let mut fixture = Fixture::new(Corruption::None);
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let member = fixture.member;
    let objects = [ScoopLirObjectCandidateV1::new(
        member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_scoop_lir_stackmaps_v1(fixture.builtins, fixture.semantic_plan, &objects),
        Err(ScoopLirStackmapValidationError::PhysicalSection {
            member,
            source: ObjectStackmapSectionError::ObjectBytesMismatch,
        })
    );
}
