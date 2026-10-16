use scoop_lir::{CBridgeProductionSetV1, GeneratedBridgePlanSetV1, ObjectSymbolSurfaceV1};

use super::*;
use crate::{
    GeneratedCBridgeObjectCandidateV1, PlannedStrongObjectSymbolRoleV1,
    PlannedStrongObjectSymbolSetV1, read_generated_c_objects_v1,
};

use super::super::generated_c_objects::tests::{fixture, member_plan, profile};
use super::super::symbol_verification::tests::object_for_plan_with_deployment;

const MINIMUM_OS: u32 = 0x000d_0100;
const SDK: u32 = 0x000e_0200;

#[test]
fn verifies_the_complete_mixed_builtin_member_set() {
    let fixture = fixture(Some("native_bridge"));
    let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&fixture.foundation).unwrap();
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
    let production_proof =
        read_generated_c_objects_v1(production, &profile, &bridge_objects).unwrap();

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

fn symbol_plan(
    foundation: &scoop_lir::ConeLirFoundation,
    members: &PlannedLinkObjectMemberSetV1,
) -> PlannedStrongObjectSymbolSetV1 {
    let surface = ObjectSymbolSurfaceV1::from_foundation(foundation).unwrap();
    PlannedStrongObjectSymbolSetV1::new(scoop_target_data::DARWIN_AARCH64, &surface, members)
        .unwrap()
}

fn canonical_value(role: PlannedStrongObjectSymbolRoleV1) -> u64 {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 4,
    }
}
