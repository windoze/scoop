use scoop_lir::{
    CBridgeProductionSetV1, GeneratedBridgePlanSetV1, LirTargetProfile, StrongObjectSymbolSurfaceV1,
};

use super::*;
use crate::{
    GeneratedCBridgeObjectCandidateV1, PlannedStrongObjectSymbolRoleV1,
    PlannedStrongObjectSymbolSetV1, verify_c_bridge_production_envelopes_v1,
};

use super::super::c_bridge_production::tests::{fixture, member_plan, profile};
use super::super::symbol_verification::tests::object_for_plan_with_deployment;

const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;

#[test]
fn verifies_the_complete_mixed_builtin_member_set() {
    let fixture = fixture(Some("native_bridge"));
    let bridge_plan =
        GeneratedBridgePlanSetV1::from_odr_free_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let symbol_plan = symbol_plan(&fixture.foundation, &member_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);

    let scoop_member = member_plan.scoop_lir_members()[0].member_id();
    let scoop_bytes = object_for_plan_with_deployment(
        symbol_plan.member(scoop_member).unwrap(),
        canonical_value,
        &[],
        None,
        &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0],
    )
    .bytes;
    let bridge_member = member_plan.generated_bridge_members()[0].member_id();
    let bridge_bytes = object_for_plan_with_deployment(
        symbol_plan.member(bridge_member).unwrap(),
        canonical_value,
        &[],
        Some((MINIMUM_OS, SDK, &[])),
        &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0],
    )
    .bytes;
    let scoop_objects = [ScoopLirObjectCandidateV1::new(scoop_member, &scoop_bytes)];
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

    let verified = verify_builtin_object_strong_relocations_v1(
        &member_plan,
        &symbol_plan,
        &scoop_objects,
        production_proof,
        &bridge_objects,
    )
    .unwrap();

    assert_eq!(verified.producer(), fixture.foundation.producer());
    assert_eq!(verified.strong_relocations().members().len(), 2);
    assert!(verified.strong_relocations().bindings().is_empty());
    assert_eq!(verified.c_bridge_production().members().len(), 1);
    let mut expected_profiles = vec![
        (
            scoop_member,
            super::super::BuiltinLinkObjectSectionProfileV1::ScoopLir,
        ),
        (
            bridge_member,
            super::super::BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
        ),
    ];
    expected_profiles.sort_unstable_by_key(|(member, _)| *member);
    assert_eq!(
        verified
            .strong_relocations()
            .members()
            .iter()
            .map(|member| (member.member(), member.definitions().sections().profile()))
            .collect::<Vec<_>>(),
        expected_profiles
    );
}

#[test]
fn rejects_missing_scoop_members_before_partial_verification() {
    let fixture = fixture(None);
    let bridge_plan =
        GeneratedBridgePlanSetV1::from_odr_free_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let symbol_plan = symbol_plan(&fixture.foundation, &member_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let production_proof = verify_c_bridge_production_envelopes_v1(
        bridge_plan.clone(),
        production,
        &profile,
        &member_plan,
        &[],
    )
    .unwrap();
    let missing = member_plan.scoop_lir_members()[0].member_id();

    assert!(matches!(
        verify_builtin_object_strong_relocations_v1(
            &member_plan,
            &symbol_plan,
            &[],
            production_proof,
            &[],
        ),
        Err(BuiltinObjectSetValidationError::MissingMember {
            kind: MemberSetKind::ScoopObjects,
            member,
        }) if member == missing
    ));
}

#[test]
fn rejects_capability_confusion_and_bytes_changed_after_bridge_proof() {
    let fixture = fixture(Some("native_bridge"));
    let bridge_plan =
        GeneratedBridgePlanSetV1::from_odr_free_foundation(&fixture.foundation).unwrap();
    let member_plan = member_plan(&fixture, &bridge_plan);
    let symbol_plan = symbol_plan(&fixture.foundation, &member_plan);
    let profile = profile("clang-2100.1.1.101", MINIMUM_OS, SDK);
    let scoop_member = member_plan.scoop_lir_members()[0].member_id();
    let bridge_member = member_plan.generated_bridge_members()[0].member_id();
    let scoop_bytes = object_for_plan_with_deployment(
        symbol_plan.member(scoop_member).unwrap(),
        canonical_value,
        &[],
        Some((MINIMUM_OS, SDK, &[])),
        &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0],
    )
    .bytes;
    let bridge_bytes = object_for_plan_with_deployment(
        symbol_plan.member(bridge_member).unwrap(),
        canonical_value,
        &[],
        Some((MINIMUM_OS, SDK, &[])),
        &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0],
    )
    .bytes;
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
    let wrong_scoop = [ScoopLirObjectCandidateV1::new(scoop_member, &scoop_bytes)];

    assert!(matches!(
        verify_builtin_object_strong_relocations_v1(
            &member_plan,
            &symbol_plan,
            &wrong_scoop,
            production_proof.clone(),
            &bridge_objects,
        ),
        Err(BuiltinObjectSetValidationError::ScoopEnvelope {
            member,
            source: super::super::ScoopLirObjectEnvelopeValidationError::UnexpectedDeployment(_),
        }) if member == scoop_member
    ));

    let valid_scoop_bytes = object_for_plan_with_deployment(
        symbol_plan.member(scoop_member).unwrap(),
        canonical_value,
        &[],
        None,
        &[0xaa, 0xbb, 0xcc, 0xdd, 0, 0, 0, 0],
    )
    .bytes;
    let valid_scoop = [ScoopLirObjectCandidateV1::new(
        scoop_member,
        &valid_scoop_bytes,
    )];
    let mut changed_bridge_bytes = bridge_bytes.clone();
    let last = changed_bridge_bytes.len() - 1;
    changed_bridge_bytes[last] ^= 1;
    let changed_bridge = [GeneratedCBridgeObjectCandidateV1::new(
        bridge_member,
        &changed_bridge_bytes,
    )];
    assert!(matches!(
        verify_builtin_object_strong_relocations_v1(
            &member_plan,
            &symbol_plan,
            &valid_scoop,
            production_proof,
            &changed_bridge,
        ),
        Err(BuiltinObjectSetValidationError::StrongDefinitions {
            member,
            source: super::super::StrongObjectDefinitionValidationError::ObjectBytesMismatch,
        }) if member == bridge_member
    ));
}

fn symbol_plan(
    foundation: &scoop_lir::OdrFreeLirFoundation,
    members: &PlannedLinkObjectMemberSetV1,
) -> PlannedStrongObjectSymbolSetV1 {
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(foundation).unwrap();
    PlannedStrongObjectSymbolSetV1::new(LirTargetProfile::DARWIN_AARCH64, &surface, members)
        .unwrap()
}

fn canonical_value(role: PlannedStrongObjectSymbolRoleV1) -> u64 {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 4,
    }
}
