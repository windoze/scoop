//! Bind both built-in codegen object sets to canonical `.slib` member identities.

use std::fmt;
use std::path::PathBuf;

use scoop_codegen::{
    EmittedGeneratedCBridgeObjectSetV1, EmittedStrongObjectMemberKindV1, EmittedStrongObjectSetV1,
    ProvisionalStrongDigestPatchLocationV1,
};
use scoop_lir::{
    CBridgeProductionSetV1, CBridgeToolchainProfileV1, GeneratedBridgeUnitId, LirTargetProfile,
    ObjectDefinitionPlanId, OdrFreeLirFoundation, StrongProducerUnitPartitionV1,
    StrongProductionSectionV1,
};
use scoop_slib::{
    BuiltinObjectSetValidationError, CBridgeProductionEnvelopeValidationError,
    CanonicalGeneratedBridgeObjectUnitSetV1, CanonicalScoopLirObjectUnitSetV1,
    DigestPatchSiteValidationError, GeneratedCBridgeObjectCandidateV1,
    LinkObjectMemberSetPlanError, ObjectUnitSetError, PlannedGeneratedBridgeObjectMemberV1,
    PlannedLinkObjectMemberSetV1, PlannedScoopLirObjectMemberV1, PlannedStrongObjectSymbolSetV1,
    ProvisionalDigestPatchSiteV1, ScoopLirObjectCandidateV1, ScoopLirStackmapValidationError,
    SlibMemberId, StrongCallableRegistrationObjectFingerprintError,
    StrongCallableRegistrationValidationError,
    StrongImmortalObjectRegistrationObjectFingerprintError,
    StrongImmortalObjectRegistrationValidationError,
    StrongInitializationRegistrationObjectFingerprintError,
    StrongInitializationRegistrationValidationError, StrongObjectSymbolPlanningError,
    StrongSafepointFingerprintError, StrongSafepointRegistrationValidationError,
    StrongStaticStorageRegistrationObjectFingerprintError,
    StrongStaticStorageRegistrationValidationError, StrongTypeRegistrationObjectFingerprintError,
    StrongTypeRegistrationValidationError, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedCBridgeProductionEnvelopeSetV1, VerifiedScoopLirDigestPatchSiteSetV1,
    VerifiedScoopLirStackmapSetV1, VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    VerifiedStrongCallableRegistrationSetV1,
    VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    VerifiedStrongImmortalObjectRegistrationSetV1,
    VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    VerifiedStrongInitializationRegistrationSetV1, VerifiedStrongSafepointFingerprintSetV1,
    VerifiedStrongSafepointRegistrationSetV1,
    VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    VerifiedStrongStaticStorageRegistrationSetV1,
    VerifiedStrongTypeRegistrationObjectFingerprintSetV1, VerifiedStrongTypeRegistrationSetV1,
    compute_strong_callable_registration_object_fingerprints_v1,
    compute_strong_immortal_object_registration_object_fingerprints_v1,
    compute_strong_initialization_registration_object_fingerprints_v1,
    compute_strong_safepoint_fingerprints_v1,
    compute_strong_static_storage_registration_object_fingerprints_v1,
    compute_strong_type_registration_object_fingerprints_v1,
    verify_builtin_object_strong_relocations_v1, verify_c_bridge_production_envelopes_v1,
    verify_scoop_lir_digest_patch_sites_v1, verify_scoop_lir_stackmaps_v1,
    verify_strong_callable_registrations_v1, verify_strong_immortal_object_registrations_v1,
    verify_strong_initialization_registrations_v1, verify_strong_safepoint_registrations_v1,
    verify_strong_static_storage_registrations_v1, verify_strong_type_registrations_v1,
};

/// Immutable bytes for one codegen member after its stable `.slib` identity
/// has been derived from the exact producer unit set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedScoopLirObjectInputV1 {
    plan: PlannedScoopLirObjectMemberV1,
    bytes: Vec<u8>,
}

/// Immutable generated-C bytes after the exact singleton bridge unit has
/// been bound to its stable `.slib` member identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedGeneratedCBridgeObjectInputV1 {
    plan: PlannedGeneratedBridgeObjectMemberV1,
    bytes: Vec<u8>,
}

/// Complete built-in object production after unified member planning and
/// typed digest-site binding, ready for the `.slib` object verifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedBuiltinObjectProductionV1 {
    target: LirTargetProfile,
    foundation: OdrFreeLirFoundation,
    production: StrongProductionSectionV1,
    c_bridge_profile: CBridgeToolchainProfileV1,
    c_bridge_production: CBridgeProductionSetV1,
    member_plan: PlannedLinkObjectMemberSetV1,
    scoop_lir_members: Vec<PlannedScoopLirObjectInputV1>,
    generated_c_bridge_members: Vec<PlannedGeneratedCBridgeObjectInputV1>,
    digest_patches: Vec<ProvisionalDigestPatchSiteV1>,
}

impl PlannedBuiltinObjectProductionV1 {
    pub fn from_codegen(
        scoop_lir: &EmittedStrongObjectSetV1,
        generated_c_bridge: &EmittedGeneratedCBridgeObjectSetV1,
    ) -> Result<Self, BuiltinObjectProductionError> {
        if scoop_lir.production().generated_bridge_plan() != generated_c_bridge.sources().plan() {
            return Err(BuiltinObjectProductionError::GeneratedBridgePlanMismatch);
        }

        let mut scoop_lir_sources = Vec::with_capacity(scoop_lir.members().len());
        for member in scoop_lir.members() {
            let bytes = std::fs::read(member.path()).map_err(|source| {
                BuiltinObjectProductionError::ReadObject {
                    producer: BuiltinObjectProducerV1::ScoopLir,
                    path: member.path().to_path_buf(),
                    source,
                }
            })?;
            let digest_patches = match member.kind() {
                EmittedStrongObjectMemberKindV1::NonCallable { digest_patches, .. } => {
                    digest_patches
                        .iter()
                        .map(|materialization| {
                            UnboundDigestPatch::from_codegen(
                                materialization.location(),
                                materialization.checked_object_offset(),
                            )
                        })
                        .collect()
                }
                EmittedStrongObjectMemberKindV1::CallableBody { .. } => Vec::new(),
            };
            scoop_lir_sources.push(UnboundScoopLirObject {
                units: member.units().definition_plans().to_vec(),
                bytes,
                digest_patches,
            });
        }

        let mut generated_c_bridge_sources = Vec::with_capacity(generated_c_bridge.members().len());
        for member in generated_c_bridge.members() {
            let bytes = std::fs::read(member.object_path()).map_err(|source| {
                BuiltinObjectProductionError::ReadObject {
                    producer: BuiltinObjectProducerV1::GeneratedCBridge,
                    path: member.object_path().to_path_buf(),
                    source,
                }
            })?;
            generated_c_bridge_sources.push(UnboundGeneratedCBridgeObject {
                unit: member.unit(),
                bytes,
            });
        }

        let bindings = plan_objects(
            scoop_lir.partition().producer_units(),
            scoop_lir_sources,
            generated_c_bridge_sources,
        )?;
        if bindings.digest_patches.is_empty() {
            return Err(BuiltinObjectProductionError::EmptyDigestMaterializationSet);
        }
        let c_bridge_profile = generated_c_bridge.profile().clone();
        let c_bridge_production = generated_c_bridge.production().clone();
        Ok(Self {
            target: scoop_lir.target(),
            foundation: scoop_lir.foundation().clone(),
            production: scoop_lir.production().clone(),
            c_bridge_profile,
            c_bridge_production,
            member_plan: bindings.member_plan,
            scoop_lir_members: bindings.scoop_lir_members,
            generated_c_bridge_members: bindings.generated_c_bridge_members,
            digest_patches: bindings.digest_patches,
        })
    }

    pub const fn production(&self) -> &StrongProductionSectionV1 {
        &self.production
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub const fn foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundation
    }

    pub const fn c_bridge_profile(&self) -> &CBridgeToolchainProfileV1 {
        &self.c_bridge_profile
    }

    pub const fn c_bridge_production(&self) -> &CBridgeProductionSetV1 {
        &self.c_bridge_production
    }

    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    fn c_bridge_candidates(&self) -> Vec<GeneratedCBridgeObjectCandidateV1<'_>> {
        self.generated_c_bridge_members
            .iter()
            .map(|member| {
                GeneratedCBridgeObjectCandidateV1::new(member.plan.member_id(), &member.bytes)
            })
            .collect()
    }

    fn scoop_lir_candidates(&self) -> Vec<ScoopLirObjectCandidateV1<'_>> {
        self.scoop_lir_members
            .iter()
            .map(|member| ScoopLirObjectCandidateV1::new(member.plan.member_id(), &member.bytes))
            .collect()
    }

    pub fn verify_c_bridge_envelopes(
        self,
    ) -> Result<CBridgeEnvelopeVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let proof = {
            let candidates = self.c_bridge_candidates();
            verify_c_bridge_production_envelopes_v1(
                self.production.generated_bridge_plan().clone(),
                self.c_bridge_production.clone(),
                &self.c_bridge_profile,
                &self.member_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::CBridgeEnvelopes)?
        };
        Ok(CBridgeEnvelopeVerifiedObjectProductionV1 {
            production: self,
            proof,
        })
    }
}

/// Planned bytes paired with the generated-C production/envelope proof that
/// authorizes the remaining built-in object verifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBridgeEnvelopeVerifiedObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    proof: VerifiedCBridgeProductionEnvelopeSetV1,
}

impl CBridgeEnvelopeVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn proof(&self) -> &VerifiedCBridgeProductionEnvelopeSetV1 {
        &self.proof
    }

    pub fn verify_strong_relocations(
        self,
    ) -> Result<StrongRelocationVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            proof: c_bridge_proof,
        } = self;
        let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
            production.target,
            production.production.canonical_definitions(),
            &production.member_plan,
        )
        .map_err(BuiltinObjectProductionError::StrongSymbolPlan)?;
        let proof = {
            let scoop_lir_candidates = production.scoop_lir_candidates();
            let c_bridge_candidates = production.c_bridge_candidates();
            verify_builtin_object_strong_relocations_v1(
                &production.member_plan,
                &symbol_plan,
                &scoop_lir_candidates,
                c_bridge_proof,
                &c_bridge_candidates,
            )
            .map_err(BuiltinObjectProductionError::StrongRelocations)?
        };
        Ok(StrongRelocationVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            proof,
        })
    }
}

/// Unified proof that all provisional built-in members satisfy their exact
/// strong symbol, atom-range, and relocation plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongRelocationVerifiedObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    proof: VerifiedBuiltinObjectStrongRelocationSetV1,
}

impl StrongRelocationVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn proof(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.proof
    }

    pub fn verify_digest_patch_sites(
        self,
    ) -> Result<DigestPatchVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            proof: strong_relocations,
        } = self;
        let proof = {
            let candidates = production.scoop_lir_candidates();
            verify_scoop_lir_digest_patch_sites_v1(
                strong_relocations,
                &production.foundation,
                production.production.digest_finalization_plan().clone(),
                &candidates,
                &production.digest_patches,
            )
            .map_err(BuiltinObjectProductionError::DigestPatchSites)?
        };
        Ok(DigestPatchVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            proof,
        })
    }
}

/// Proof that every planned digest intent owns one in-atom,
/// relocation-free, provisionally zero Scoop object slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigestPatchVerifiedObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    proof: VerifiedScoopLirDigestPatchSiteSetV1,
}

impl DigestPatchVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn proof(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.proof
    }

    pub fn verify_stackmaps(
        self,
    ) -> Result<StackmapVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            proof: digest_patch_sites,
        } = self;
        let stackmaps = {
            let candidates = production.scoop_lir_candidates();
            let semantic_plan = production
                .production
                .registration_production()
                .safepoint_semantics();
            verify_scoop_lir_stackmaps_v1(
                digest_patch_sites.builtins().clone(),
                semantic_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::Stackmaps)?
        };
        Ok(StackmapVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            digest_patch_sites,
            stackmaps,
        })
    }
}

/// Complete normalized stackmap proof paired with the digest-site proof from
/// the same provisional Scoop object bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackmapVerifiedObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    digest_patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
}

impl StackmapVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn digest_patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.digest_patch_sites
    }

    pub const fn stackmaps(&self) -> &VerifiedScoopLirStackmapSetV1 {
        &self.stackmaps
    }

    pub fn verify_registration_objects(
        self,
    ) -> Result<RegistrationObjectVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            digest_patch_sites,
            stackmaps,
        } = self;
        let registration_production = production.production.registration_production();
        let safepoint_plan = registration_production.safepoints().clone();
        let callable_plan = registration_production.callables().clone();
        let type_plan = registration_production.types().clone();
        let immortal_object_plan = registration_production.immortal_objects().clone();
        let static_storage_plan = registration_production.static_storages().clone();
        let initialization_plan = registration_production.initialization_units().clone();

        let (
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
        ) = {
            let candidates = production.scoop_lir_candidates();
            let safepoint_registrations = verify_strong_safepoint_registrations_v1(
                stackmaps,
                digest_patch_sites.clone(),
                safepoint_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::SafepointRegistrations)?;
            let callable_registrations = verify_strong_callable_registrations_v1(
                digest_patch_sites.clone(),
                callable_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::CallableRegistrations)?;
            let type_registrations = verify_strong_type_registrations_v1(
                digest_patch_sites.clone(),
                type_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::TypeRegistrations)?;
            let immortal_object_registrations = verify_strong_immortal_object_registrations_v1(
                digest_patch_sites.clone(),
                immortal_object_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::ImmortalObjectRegistrations)?;
            let static_storage_registrations = verify_strong_static_storage_registrations_v1(
                digest_patch_sites.clone(),
                static_storage_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::StaticStorageRegistrations)?;
            let initialization_registrations = verify_strong_initialization_registrations_v1(
                digest_patch_sites,
                initialization_plan,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::InitializationRegistrations)?;
            (
                safepoint_registrations,
                callable_registrations,
                type_registrations,
                immortal_object_registrations,
                static_storage_registrations,
                initialization_registrations,
            )
        };

        Ok(RegistrationObjectVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
        })
    }
}

/// Atomic proof that all six strong registration families match the same
/// provisional Scoop object set and digest graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationObjectVerifiedObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    safepoint_registrations: VerifiedStrongSafepointRegistrationSetV1,
    callable_registrations: VerifiedStrongCallableRegistrationSetV1,
    type_registrations: VerifiedStrongTypeRegistrationSetV1,
    immortal_object_registrations: VerifiedStrongImmortalObjectRegistrationSetV1,
    static_storage_registrations: VerifiedStrongStaticStorageRegistrationSetV1,
    initialization_registrations: VerifiedStrongInitializationRegistrationSetV1,
}

impl RegistrationObjectVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn safepoint_registrations(&self) -> &VerifiedStrongSafepointRegistrationSetV1 {
        &self.safepoint_registrations
    }

    pub const fn callable_registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callable_registrations
    }

    pub const fn type_registrations(&self) -> &VerifiedStrongTypeRegistrationSetV1 {
        &self.type_registrations
    }

    pub const fn immortal_object_registrations(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationSetV1 {
        &self.immortal_object_registrations
    }

    pub const fn static_storage_registrations(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationSetV1 {
        &self.static_storage_registrations
    }

    pub const fn initialization_registrations(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationSetV1 {
        &self.initialization_registrations
    }

    pub fn fingerprint_registration_object_leaves(
        self,
    ) -> Result<RegistrationObjectLeafFingerprintedProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
        } = self;
        let (
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        ) = {
            let candidates = production.scoop_lir_candidates();
            let safepoints =
                compute_strong_safepoint_fingerprints_v1(safepoint_registrations, &candidates)
                    .map_err(BuiltinObjectProductionError::SafepointFingerprints)?;
            let callable_registration_objects =
                compute_strong_callable_registration_object_fingerprints_v1(
                    callable_registrations,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::CallableRegistrationObjectFingerprints)?;
            let type_registration_objects =
                compute_strong_type_registration_object_fingerprints_v1(
                    type_registrations,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::TypeRegistrationObjectFingerprints)?;
            let immortal_object_registration_objects =
                compute_strong_immortal_object_registration_object_fingerprints_v1(
                    immortal_object_registrations,
                    &candidates,
                )
                .map_err(
                    BuiltinObjectProductionError::ImmortalObjectRegistrationObjectFingerprints,
                )?;
            let static_storage_registration_objects =
                compute_strong_static_storage_registration_object_fingerprints_v1(
                    static_storage_registrations,
                    &candidates,
                )
                .map_err(
                    BuiltinObjectProductionError::StaticStorageRegistrationObjectFingerprints,
                )?;
            let initialization_registration_objects =
                compute_strong_initialization_registration_object_fingerprints_v1(
                    initialization_registrations,
                    &candidates,
                )
                .map_err(
                    BuiltinObjectProductionError::InitializationRegistrationObjectFingerprints,
                )?;
            (
                safepoints,
                callable_registration_objects,
                type_registration_objects,
                immortal_object_registration_objects,
                static_storage_registration_objects,
                initialization_registration_objects,
            )
        };

        Ok(RegistrationObjectLeafFingerprintedProductionV1 {
            production,
            symbol_plan,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        })
    }
}

/// All six registration-object leaf families fingerprinted from one exact
/// provisional Scoop object set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationObjectLeafFingerprintedProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callable_registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
}

impl RegistrationObjectLeafFingerprintedProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registration_objects(
        &self,
    ) -> &VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        &self.callable_registration_objects
    }

    pub const fn type_registration_objects(
        &self,
    ) -> &VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        &self.type_registration_objects
    }

    pub const fn immortal_object_registration_objects(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
        &self.immortal_object_registration_objects
    }

    pub const fn static_storage_registration_objects(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
        &self.static_storage_registration_objects
    }

    pub const fn initialization_registration_objects(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationObjectFingerprintSetV1 {
        &self.initialization_registration_objects
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinObjectProducerV1 {
    ScoopLir,
    GeneratedCBridge,
}

#[derive(Clone, Debug)]
struct UnboundScoopLirObject {
    units: Vec<ObjectDefinitionPlanId>,
    bytes: Vec<u8>,
    digest_patches: Vec<UnboundDigestPatch>,
}

#[derive(Clone, Debug)]
struct UnboundGeneratedCBridgeObject {
    unit: GeneratedBridgeUnitId,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct UnboundDigestPatch {
    intent: scoop_identity::DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    checked_object_offset: u64,
    width_bytes: u8,
}

impl UnboundDigestPatch {
    fn from_codegen(location: ProvisionalStrongDigestPatchLocationV1, offset: u64) -> Self {
        Self {
            intent: location.intent(),
            definition: location.definition(),
            checked_object_offset: offset,
            width_bytes: location.width_bytes(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlannedObjectBindings {
    member_plan: PlannedLinkObjectMemberSetV1,
    scoop_lir_members: Vec<PlannedScoopLirObjectInputV1>,
    generated_c_bridge_members: Vec<PlannedGeneratedCBridgeObjectInputV1>,
    digest_patches: Vec<ProvisionalDigestPatchSiteV1>,
}

fn plan_objects(
    producer_units: &StrongProducerUnitPartitionV1,
    scoop_lir_sources: Vec<UnboundScoopLirObject>,
    generated_c_bridge_sources: Vec<UnboundGeneratedCBridgeObject>,
) -> Result<PlannedObjectBindings, BuiltinObjectProductionError> {
    let scoop_lir_unit_sets = scoop_lir_sources
        .iter()
        .map(|source| CanonicalScoopLirObjectUnitSetV1::new(source.units.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| BuiltinObjectProductionError::Units {
            producer: BuiltinObjectProducerV1::ScoopLir,
            source,
        })?;
    let generated_bridge_unit_sets = generated_c_bridge_sources
        .iter()
        .map(|source| CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![source.unit]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| BuiltinObjectProductionError::Units {
            producer: BuiltinObjectProducerV1::GeneratedCBridge,
            source,
        })?;
    let member_plan = PlannedLinkObjectMemberSetV1::new(
        producer_units,
        scoop_lir_unit_sets,
        generated_bridge_unit_sets,
    )
    .map_err(BuiltinObjectProductionError::MemberPlan)?;

    let mut scoop_lir_members = Vec::with_capacity(scoop_lir_sources.len());
    let mut digest_patches = Vec::new();
    for source in scoop_lir_sources {
        let member = scoop_member_for_units(&member_plan, &source.units)?;
        for patch in source.digest_patches {
            let assigned = member_plan.member_for_definition(patch.definition).ok_or(
                BuiltinObjectProductionError::UnassignedPatchDefinition(patch.definition),
            )?;
            if assigned != member.member_id() {
                return Err(BuiltinObjectProductionError::PatchMemberMismatch {
                    definition: patch.definition,
                    expected: assigned,
                    actual: member.member_id(),
                });
            }
            digest_patches.push(ProvisionalDigestPatchSiteV1::new(
                patch.intent,
                member.member_id(),
                patch.checked_object_offset,
                patch.width_bytes,
            ));
        }
        scoop_lir_members.push(PlannedScoopLirObjectInputV1 {
            plan: member.clone(),
            bytes: source.bytes,
        });
    }
    scoop_lir_members.sort_unstable_by_key(|member| member.plan.member_id());

    let mut generated_c_bridge_members = Vec::with_capacity(generated_c_bridge_sources.len());
    for source in generated_c_bridge_sources {
        let member = generated_bridge_member_for_unit(&member_plan, source.unit)?;
        generated_c_bridge_members.push(PlannedGeneratedCBridgeObjectInputV1 {
            plan: member.clone(),
            bytes: source.bytes,
        });
    }
    generated_c_bridge_members.sort_unstable_by_key(|member| member.plan.member_id());

    digest_patches.sort_unstable_by_key(|patch| patch.intent());
    if let Some(pair) = digest_patches
        .windows(2)
        .find(|pair| pair[0].intent() == pair[1].intent())
    {
        return Err(BuiltinObjectProductionError::DuplicateDigestIntent(
            pair[0].intent(),
        ));
    }
    Ok(PlannedObjectBindings {
        member_plan,
        scoop_lir_members,
        generated_c_bridge_members,
        digest_patches,
    })
}

fn scoop_member_for_units<'plan>(
    member_plan: &'plan PlannedLinkObjectMemberSetV1,
    units: &[ObjectDefinitionPlanId],
) -> Result<&'plan PlannedScoopLirObjectMemberV1, BuiltinObjectProductionError> {
    let first = *units
        .first()
        .ok_or(BuiltinObjectProductionError::EmptyObjectUnits)?;
    let member = member_plan.member_for_definition(first).ok_or(
        BuiltinObjectProductionError::UnassignedObjectDefinition(first),
    )?;
    if units
        .iter()
        .any(|definition| member_plan.member_for_definition(*definition) != Some(member))
    {
        return Err(BuiltinObjectProductionError::SplitObjectUnits(member));
    }
    member_plan
        .scoop_lir_members()
        .iter()
        .find(|plan| plan.member_id() == member)
        .ok_or(BuiltinObjectProductionError::MissingPlannedMember(member))
}

fn generated_bridge_member_for_unit(
    member_plan: &PlannedLinkObjectMemberSetV1,
    unit: GeneratedBridgeUnitId,
) -> Result<&PlannedGeneratedBridgeObjectMemberV1, BuiltinObjectProductionError> {
    let member = member_plan.member_for_generated_bridge_unit(unit).ok_or(
        BuiltinObjectProductionError::UnassignedGeneratedBridgeUnit(unit),
    )?;
    member_plan
        .generated_bridge_members()
        .iter()
        .find(|plan| plan.member_id() == member)
        .ok_or(BuiltinObjectProductionError::MissingPlannedMember(member))
}

#[derive(Debug)]
pub enum BuiltinObjectProductionError {
    ReadObject {
        producer: BuiltinObjectProducerV1,
        path: PathBuf,
        source: std::io::Error,
    },
    GeneratedBridgePlanMismatch,
    CBridgeEnvelopes(CBridgeProductionEnvelopeValidationError),
    StrongSymbolPlan(StrongObjectSymbolPlanningError),
    StrongRelocations(BuiltinObjectSetValidationError),
    DigestPatchSites(DigestPatchSiteValidationError),
    Stackmaps(ScoopLirStackmapValidationError),
    SafepointRegistrations(StrongSafepointRegistrationValidationError),
    CallableRegistrations(StrongCallableRegistrationValidationError),
    TypeRegistrations(StrongTypeRegistrationValidationError),
    ImmortalObjectRegistrations(StrongImmortalObjectRegistrationValidationError),
    StaticStorageRegistrations(StrongStaticStorageRegistrationValidationError),
    InitializationRegistrations(StrongInitializationRegistrationValidationError),
    SafepointFingerprints(StrongSafepointFingerprintError),
    CallableRegistrationObjectFingerprints(StrongCallableRegistrationObjectFingerprintError),
    TypeRegistrationObjectFingerprints(StrongTypeRegistrationObjectFingerprintError),
    ImmortalObjectRegistrationObjectFingerprints(
        StrongImmortalObjectRegistrationObjectFingerprintError,
    ),
    StaticStorageRegistrationObjectFingerprints(
        StrongStaticStorageRegistrationObjectFingerprintError,
    ),
    InitializationRegistrationObjectFingerprints(
        StrongInitializationRegistrationObjectFingerprintError,
    ),
    Units {
        producer: BuiltinObjectProducerV1,
        source: ObjectUnitSetError,
    },
    MemberPlan(LinkObjectMemberSetPlanError),
    EmptyObjectUnits,
    UnassignedObjectDefinition(ObjectDefinitionPlanId),
    UnassignedGeneratedBridgeUnit(GeneratedBridgeUnitId),
    SplitObjectUnits(SlibMemberId),
    MissingPlannedMember(SlibMemberId),
    UnassignedPatchDefinition(ObjectDefinitionPlanId),
    PatchMemberMismatch {
        definition: ObjectDefinitionPlanId,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    DuplicateDigestIntent(scoop_identity::DigestPatchIntentId),
    EmptyDigestMaterializationSet,
}

impl fmt::Display for BuiltinObjectProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid built-in object production: {self:?}")
    }
}

impl std::error::Error for BuiltinObjectProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadObject { source, .. } => Some(source),
            Self::CBridgeEnvelopes(source) => Some(source),
            Self::StrongSymbolPlan(source) => Some(source),
            Self::StrongRelocations(source) => Some(source),
            Self::DigestPatchSites(source) => Some(source),
            Self::Stackmaps(source) => Some(source),
            Self::SafepointRegistrations(source) => Some(source),
            Self::CallableRegistrations(source) => Some(source),
            Self::TypeRegistrations(source) => Some(source),
            Self::ImmortalObjectRegistrations(source) => Some(source),
            Self::StaticStorageRegistrations(source) => Some(source),
            Self::InitializationRegistrations(source) => Some(source),
            Self::SafepointFingerprints(source) => Some(source),
            Self::CallableRegistrationObjectFingerprints(source) => Some(source),
            Self::TypeRegistrationObjectFingerprints(source) => Some(source),
            Self::ImmortalObjectRegistrationObjectFingerprints(source) => Some(source),
            Self::StaticStorageRegistrationObjectFingerprints(source) => Some(source),
            Self::InitializationRegistrationObjectFingerprints(source) => Some(source),
            Self::Units { source, .. } => Some(source),
            Self::MemberPlan(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
