mod macho;
mod record;
mod semantic;

use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    ConeLirFoundation, DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
    GeneratedBridgePlanSetV1, ObjectSymbolSurfaceV1, ProducerUnitPartitionV1,
    StrongCallableRegistrationPlanSetV1, StrongInitializationUnitRegistrationPlanSetV1,
    StrongSafepointSemanticPlanSetV1,
};

use crate::SlibMemberId;
use crate::link_object::{
    CanonicalScoopLirObjectUnitSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolSetV1, ScoopLirObjectCandidateV1,
    ScoopStackmapSetV1, VerifiedBuiltinObjectStrongRelocationSetV1, read_generated_c_objects_v1,
    read_scoop_stackmaps_v1, verify_builtin_object_strong_relocations_v1,
};

pub(super) struct Fixture {
    pub(super) builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    pub(super) foundation: ConeLirFoundation,
    pub(super) plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(super) callable_plan: StrongCallableRegistrationPlanSetV1,
    pub(super) safepoints: StrongSafepointSemanticPlanSetV1,
    pub(super) member: SlibMemberId,
    pub(super) object_bytes: Vec<u8>,
}

impl Fixture {
    pub(super) fn new(lazy: bool) -> Self {
        let inputs = semantic::inputs(lazy);
        let bridge_plan = GeneratedBridgePlanSetV1::from_foundation(&inputs.foundation).unwrap();
        let partition = ProducerUnitPartitionV1::from_foundation(&inputs.foundation).unwrap();
        let member_plan = PlannedLinkObjectMemberSetV1::new(
            scoop_target_data::DARWIN_AARCH64,
            &partition,
            vec![CanonicalScoopLirObjectUnitSetV1::new(inputs.definitions).unwrap()],
            Vec::new(),
        )
        .unwrap();
        let surface = ObjectSymbolSurfaceV1::from_foundation(&inputs.foundation).unwrap();
        let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
            scoop_target_data::DARWIN_AARCH64,
            &surface,
            &member_plan,
        )
        .unwrap();
        let member = member_plan.scoop_lir_members()[0].member_id();
        let object = macho::object_bytes(
            symbol_plan.member(member).unwrap(),
            &inputs.plan,
            &inputs.callable_plan,
        );
        let profile = c_bridge_profile();
        let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
        let production_proof = read_generated_c_objects_v1(production, &profile, &[]).unwrap();
        let objects = [ScoopLirObjectCandidateV1::new(member, &object.bytes)];
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
            foundation: inputs.foundation,
            plan: inputs.plan,
            callable_plan: inputs.callable_plan,
            safepoints: inputs.safepoints,
            member,
            object_bytes: object.bytes,
        }
    }

    pub(super) fn objects(&self) -> [ScoopLirObjectCandidateV1<'_>; 1] {
        [ScoopLirObjectCandidateV1::new(
            self.member,
            &self.object_bytes,
        )]
    }

    pub(super) fn object_data(&self) -> VerifiedBuiltinObjectStrongRelocationSetV1 {
        self.builtins.clone()
    }

    pub(super) fn verified_stackmaps(&self) -> ScoopStackmapSetV1 {
        read_scoop_stackmaps_v1(
            self.builtins.clone(),
            self.safepoints.clone(),
            &self.objects(),
        )
        .unwrap()
    }

    pub(super) fn undefined_requirements(&self) -> CanonicalUndefinedSymbolRequirementSetV1 {
        crate::link_object::undefined_requirements::tests::empty_final_requirements_for_strong(
            self.builtins.strong_relocations().clone(),
        )
    }
}

fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
    scoop_target_data::darwin_c_bridge_profile(
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
