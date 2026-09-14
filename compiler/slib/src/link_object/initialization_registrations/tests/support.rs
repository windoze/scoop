mod macho;
mod semantic;

use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, GeneratedBridgePlanSetV1,
    LirTargetProfile, OdrFreeLirFoundation, StrongCallableRegistrationPlanSetV1,
    StrongDigestFinalizationPlanV1, StrongInitializationUnitRegistrationPlanSetV1,
    StrongObjectSymbolSurfaceV1, StrongProducerUnitPartitionV1, StrongSafepointSemanticPlanSetV1,
};

use crate::SlibMemberId;
use crate::link_object::{
    CanonicalScoopLirObjectUnitSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolSetV1, ProvisionalDigestPatchSiteV1,
    ScoopLirObjectCandidateV1, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedScoopLirDigestPatchSiteSetV1, VerifiedScoopLirStackmapSetV1,
    verify_builtin_object_strong_relocations_v1, verify_c_bridge_production_envelopes_v1,
    verify_scoop_lir_digest_patch_sites_v1, verify_scoop_lir_stackmaps_v1,
};

#[derive(Clone, Copy)]
pub(super) enum Corruption {
    None,
    CellByte,
    CoordinatorByte,
    RegistrationByte,
    StorageRegistrationTarget,
    GatewayTarget,
    DiagnosticByte,
}

pub(super) struct Fixture {
    pub(super) builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    pub(super) foundation: OdrFreeLirFoundation,
    pub(super) digest_plan: StrongDigestFinalizationPlanV1,
    pub(super) plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(super) callable_plan: StrongCallableRegistrationPlanSetV1,
    pub(super) safepoints: StrongSafepointSemanticPlanSetV1,
    pub(super) provisional_patch_sites: Vec<ProvisionalDigestPatchSiteV1>,
    pub(super) member: SlibMemberId,
    pub(super) object_bytes: Vec<u8>,
}

impl Fixture {
    pub(super) fn new(lazy: bool, corruption: Corruption) -> Self {
        let inputs = semantic::inputs(lazy);
        let bridge_plan =
            GeneratedBridgePlanSetV1::from_odr_free_foundation(&inputs.foundation).unwrap();
        let partition =
            StrongProducerUnitPartitionV1::from_odr_free_foundation(&inputs.foundation).unwrap();
        let member_plan = PlannedLinkObjectMemberSetV1::new(
            &partition,
            vec![CanonicalScoopLirObjectUnitSetV1::new(inputs.definitions).unwrap()],
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
        let object = macho::object_bytes(
            symbol_plan.member(member).unwrap(),
            &inputs.plan,
            &inputs.callable_plan,
            corruption,
        );
        let profile = c_bridge_profile();
        let production = CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
        let production_proof = verify_c_bridge_production_envelopes_v1(
            bridge_plan.clone(),
            production,
            &profile,
            &member_plan,
            &[],
        )
        .unwrap();
        let objects = [ScoopLirObjectCandidateV1::new(member, &object.bytes)];
        let builtins = verify_builtin_object_strong_relocations_v1(
            &member_plan,
            &symbol_plan,
            &objects,
            production_proof,
            &[],
        )
        .unwrap();
        let provisional_patch_sites = object
            .patch_offsets
            .iter()
            .map(|(intent, offset)| ProvisionalDigestPatchSiteV1::new(*intent, member, *offset, 32))
            .collect();
        Self {
            builtins,
            foundation: inputs.foundation,
            digest_plan: inputs.digest_plan,
            plan: inputs.plan,
            callable_plan: inputs.callable_plan,
            safepoints: inputs.safepoints,
            provisional_patch_sites,
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

    pub(super) fn verified_patch_sites(&self) -> VerifiedScoopLirDigestPatchSiteSetV1 {
        verify_scoop_lir_digest_patch_sites_v1(
            self.builtins.clone(),
            &self.foundation,
            self.digest_plan.clone(),
            &self.objects(),
            &self.provisional_patch_sites,
        )
        .unwrap()
    }

    pub(super) fn verified_stackmaps(&self) -> VerifiedScoopLirStackmapSetV1 {
        verify_scoop_lir_stackmaps_v1(
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
