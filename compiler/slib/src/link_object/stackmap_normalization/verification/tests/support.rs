use scoop_identity::ConeIdentity;
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, GeneratedBridgePlanSetV1,
    LirTargetProfile, StrongObjectSymbolSurfaceV1, StrongProducerUnitPartitionV1,
    StrongSafepointSemanticPlanSetV1,
};

use crate::SlibMemberId;
use crate::link_object::{
    CanonicalScoopLirObjectUnitSetV1, PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolSetV1,
    ScoopLirObjectCandidateV1, VerifiedBuiltinObjectStrongRelocationSetV1,
    verify_builtin_object_strong_relocations_v1, verify_c_bridge_production_envelopes_v1,
};

mod macho;
mod semantic;

#[derive(Clone, Copy)]
pub(super) enum Corruption {
    None,
    UnknownSafepoint,
    WrongStackmapAtomRole,
    NonCallReturnPc,
    MissingFrameChain,
}

pub(super) struct Fixture {
    pub(super) builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    pub(super) semantic_plan: StrongSafepointSemanticPlanSetV1,
    pub(super) member: SlibMemberId,
    pub(super) object_bytes: Vec<u8>,
}

impl Fixture {
    pub(super) fn new(corruption: Corruption) -> Self {
        let inputs = semantic::inputs(corruption);
        let semantic_plan = StrongSafepointSemanticPlanSetV1::from_module(&inputs.module).unwrap();
        let bridge_plan =
            GeneratedBridgePlanSetV1::from_odr_free_foundation(&inputs.foundation).unwrap();
        let partition =
            StrongProducerUnitPartitionV1::from_odr_free_foundation(&inputs.foundation).unwrap();
        let member_plan = PlannedLinkObjectMemberSetV1::new(
            ConeIdentity::SINGLE_FILE,
            &partition,
            vec![CanonicalScoopLirObjectUnitSetV1::new(vec![inputs.definition]).unwrap()],
            Vec::new(),
        )
        .unwrap();
        let surface =
            StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&inputs.foundation).unwrap();
        let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
            LirTargetProfile::DARWIN_AARCH64,
            &surface,
            &member_plan,
        )
        .unwrap();
        let member = member_plan.scoop_lir_members()[0].member_id();
        let object_bytes = macho::object_bytes(
            symbol_plan.member(member).unwrap(),
            &inputs.safepoint_ids,
            corruption,
        );
        let profile = c_bridge_profile();
        let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
        let production_proof = verify_c_bridge_production_envelopes_v1(
            &bridge_plan,
            &production,
            &profile,
            &member_plan,
            &[],
        )
        .unwrap();
        let objects = [ScoopLirObjectCandidateV1::new(member, &object_bytes)];
        let builtins = verify_builtin_object_strong_relocations_v1(
            &member_plan,
            &symbol_plan,
            &objects,
            production_proof,
            &[],
        )
        .unwrap();
        Self {
            builtins,
            semantic_plan,
            member,
            object_bytes,
        }
    }
}

fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
    CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
        DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
            DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
            Vec::new(),
        )
        .unwrap(),
        AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap(),
    )
    .unwrap()
}
