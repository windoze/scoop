use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, GeneratedBridgePlanSetV1,
    ObjectSymbolSurfaceV1, ProducerUnitPartitionV1, StrongCallableRegistrationPlanSetV1,
    StrongImmortalObjectRegistrationPlanSetV1, StrongInitializationUnitRegistrationPlanSetV1,
    StrongSafepointRegistrationPlanSetV1, StrongSafepointSemanticPlanSetV1,
    StrongStaticStorageRegistrationPlanSetV1, StrongTypeRegistrationPlanSetV1,
};

use crate::SlibMemberId;
use crate::link_object::{
    CanonicalScoopLirObjectUnitSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolSetV1, ScoopLirObjectCandidateV1,
    ScoopStackmapSetV1, VerifiedBuiltinObjectStrongRelocationSetV1, read_generated_c_objects_v1,
    read_scoop_stackmaps_v1, verify_builtin_object_strong_relocations_v1,
};

pub(crate) mod macho;
pub(crate) mod semantic;

#[derive(Clone, Copy)]
pub(crate) enum Corruption {
    None,
    UnknownSafepoint,
}

pub(crate) struct Fixture {
    pub(crate) builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    pub(crate) semantic_plan: StrongSafepointSemanticPlanSetV1,
    pub(crate) canonical_callables: scoop_lir::CanonicalCallableAbisV1,
    pub(crate) canonical_shapes: scoop_lir::CanonicalShapeAbisV1,
    pub(crate) registration_plan: StrongSafepointRegistrationPlanSetV1,
    pub(crate) callable_registration_plan: StrongCallableRegistrationPlanSetV1,
    pub(crate) type_registration_plan: StrongTypeRegistrationPlanSetV1,
    pub(crate) immortal_registration_plan: StrongImmortalObjectRegistrationPlanSetV1,
    pub(crate) static_storage_registration_plan: StrongStaticStorageRegistrationPlanSetV1,
    pub(crate) initialization_registration_plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(crate) member: SlibMemberId,
    pub(crate) object_bytes: Vec<u8>,
}

impl Fixture {
    pub(crate) fn new(corruption: Corruption) -> Self {
        let inputs = semantic::inputs();
        let canonical_callables =
            scoop_lir::CanonicalCallableAbisV1::from_module(&inputs.module, &inputs.foundation)
                .unwrap();
        let canonical_shapes = scoop_lir::CanonicalShapeAbisV1::from_module(
            &inputs.module,
            &inputs.foundation,
            inputs
                .immortal_registration_plan
                .registrations()
                .iter()
                .map(|plan| plan.semantic()),
            inputs
                .static_storage_registration_plan
                .registrations()
                .iter()
                .map(|plan| plan.semantic()),
        )
        .unwrap();
        let semantic_plan = StrongSafepointSemanticPlanSetV1::from_module(&inputs.module).unwrap();
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
            semantic_plan,
            canonical_callables,
            canonical_shapes,
            registration_plan: inputs.registration_plan,
            callable_registration_plan: inputs.callable_registration_plan,
            type_registration_plan: inputs.type_registration_plan,
            immortal_registration_plan: inputs.immortal_registration_plan,
            static_storage_registration_plan: inputs.static_storage_registration_plan,
            initialization_registration_plan: inputs.initialization_registration_plan,
            member,
            object_bytes: object.bytes,
        }
    }

    pub(crate) fn verified_stackmaps(&self) -> ScoopStackmapSetV1 {
        let objects = [ScoopLirObjectCandidateV1::new(
            self.member,
            &self.object_bytes,
        )];
        read_scoop_stackmaps_v1(self.builtins.clone(), self.semantic_plan.clone(), &objects)
            .unwrap()
    }

    pub(crate) fn object_data(&self) -> VerifiedBuiltinObjectStrongRelocationSetV1 {
        self.builtins.clone()
    }

    pub(crate) fn undefined_requirements(&self) -> CanonicalUndefinedSymbolRequirementSetV1 {
        crate::link_object::undefined_requirements::tests::empty_final_requirements_for_strong(
            self.builtins.strong_relocations().clone(),
        )
    }
}

fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
    scoop_target_data::darwin_c_bridge_profile(
        scoop_target_data::DARWIN_AARCH64,
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
