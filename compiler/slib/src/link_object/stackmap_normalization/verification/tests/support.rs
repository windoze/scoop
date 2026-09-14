use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, GeneratedBridgePlanSetV1,
    LirTargetProfile, OdrFreeLirFoundation, StrongCallableRegistrationPlanSetV1,
    StrongDigestFinalizationPlanV1, StrongImmortalObjectRegistrationPlanSetV1,
    StrongInitializationUnitRegistrationPlanSetV1, StrongObjectSymbolSurfaceV1,
    StrongProducerUnitPartitionV1, StrongSafepointRegistrationPlanSetV1,
    StrongSafepointSemanticPlanSetV1, StrongStaticStorageRegistrationPlanSetV1,
    StrongTypeRegistrationPlanSetV1,
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

pub(crate) mod macho;
pub(crate) mod semantic;

#[derive(Clone, Copy)]
pub(crate) enum Corruption {
    None,
    UnknownSafepoint,
    WrongStackmapAtomRole,
    NonCallReturnPc,
    MissingFrameChain,
    RegistrationMagic,
    CallableRegistrationMagic,
    CallableEntryRelocationTarget,
    TypeRegistrationMagic,
    TypeDescriptorRelocationTarget,
    TypeDescriptorScalar,
    TypeDescriptorDiagnosticBytes,
    TypeDescriptorDiagnosticRelocationTarget,
    ImmortalRegistrationMagic,
    ImmortalObjectLength,
    ImmortalObjectDescriptorRelocationTarget,
    ImmortalObjectRelocationTarget,
    ImmortalTypeRegistrationRelocationTarget,
    StaticRegistrationMagic,
    StaticScanProgram,
    StaticStorageRelocationTarget,
    StaticInitialStorageRelocationTarget,
    StaticInitialTableRelocationTarget,
    StaticZeroedInitialState,
    StaticEncodedEmptyInitialState,
    StaticSentinelCollision,
    CoreExternalImmortalTypeRegistration,
    WritableRegistrationSection,
    RelocatedRegistration,
}

pub(crate) struct Fixture {
    pub(crate) builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    pub(crate) semantic_plan: StrongSafepointSemanticPlanSetV1,
    pub(crate) foundation: OdrFreeLirFoundation,
    pub(crate) digest_plan: StrongDigestFinalizationPlanV1,
    pub(crate) registration_plan: StrongSafepointRegistrationPlanSetV1,
    pub(crate) callable_registration_plan: StrongCallableRegistrationPlanSetV1,
    pub(crate) type_registration_plan: StrongTypeRegistrationPlanSetV1,
    pub(crate) immortal_registration_plan: StrongImmortalObjectRegistrationPlanSetV1,
    pub(crate) static_storage_registration_plan: StrongStaticStorageRegistrationPlanSetV1,
    pub(crate) initialization_registration_plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(crate) provisional_patch_sites: Vec<ProvisionalDigestPatchSiteV1>,
    pub(crate) member: SlibMemberId,
    pub(crate) object_bytes: Vec<u8>,
}

impl Fixture {
    pub(crate) fn new(corruption: Corruption) -> Self {
        let inputs = semantic::inputs(corruption);
        let semantic_plan = StrongSafepointSemanticPlanSetV1::from_module(&inputs.module).unwrap();
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
            &inputs.module,
            symbol_plan.member(member).unwrap(),
            &inputs.safepoint_ids,
            &inputs.registration_plan,
            &inputs.callable_registration_plan,
            &inputs.type_registration_plan,
            &inputs.immortal_registration_plan,
            &inputs.static_storage_registration_plan,
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
            semantic_plan,
            foundation: inputs.foundation,
            digest_plan: inputs.digest_plan,
            registration_plan: inputs.registration_plan,
            callable_registration_plan: inputs.callable_registration_plan,
            type_registration_plan: inputs.type_registration_plan,
            immortal_registration_plan: inputs.immortal_registration_plan,
            static_storage_registration_plan: inputs.static_storage_registration_plan,
            initialization_registration_plan: inputs.initialization_registration_plan,
            provisional_patch_sites,
            member,
            object_bytes: object.bytes,
        }
    }

    pub(crate) fn verified_stackmaps(&self) -> VerifiedScoopLirStackmapSetV1 {
        let objects = [ScoopLirObjectCandidateV1::new(
            self.member,
            &self.object_bytes,
        )];
        verify_scoop_lir_stackmaps_v1(self.builtins.clone(), self.semantic_plan.clone(), &objects)
            .unwrap()
    }

    pub(crate) fn verified_patch_sites(&self) -> VerifiedScoopLirDigestPatchSiteSetV1 {
        let objects = [ScoopLirObjectCandidateV1::new(
            self.member,
            &self.object_bytes,
        )];
        verify_scoop_lir_digest_patch_sites_v1(
            self.builtins.clone(),
            &self.foundation,
            self.digest_plan.clone(),
            &objects,
            &self.provisional_patch_sites,
        )
        .unwrap()
    }

    pub(crate) fn undefined_requirements(&self) -> CanonicalUndefinedSymbolRequirementSetV1 {
        match self.immortal_registration_plan.registrations()[0]
            .semantic()
            .type_registration_ref()
        {
            scoop_lir::ImmortalObjectTypeRegistrationRefV1::Local(_) => {
                crate::link_object::undefined_requirements::tests::
                    empty_final_requirements_for_strong(
                        self.builtins.strong_relocations().clone(),
                    )
            }
            scoop_lir::ImmortalObjectTypeRegistrationRefV1::CoreExternal(exact_type) => {
                crate::link_object::undefined_requirements::tests::
                    core_type_final_requirements_for_strong(
                        self.builtins.strong_relocations().clone(),
                        exact_type,
                    )
            }
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
