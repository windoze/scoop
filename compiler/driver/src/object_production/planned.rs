//! Immutable codegen inputs and canonical object member planning.

use super::*;

/// Immutable bytes for one codegen member after its stable `.slib` identity
/// has been derived from the exact producer unit set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedScoopLirObjectInputV1 {
    pub(in crate::object_production) plan: PlannedScoopLirObjectMemberV1,
    pub(in crate::object_production) bytes: Vec<u8>,
}

/// Immutable generated-C bytes after the exact singleton bridge unit has
/// been bound to its stable `.slib` member identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedGeneratedCBridgeObjectInputV1 {
    pub(in crate::object_production) plan: PlannedGeneratedBridgeObjectMemberV1,
    pub(in crate::object_production) bytes: Vec<u8>,
}

/// Complete built-in object production after unified member planning and
/// typed digest-site binding, ready for the `.slib` object verifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedBuiltinObjectProductionV1 {
    pub(in crate::object_production) target_selection: ValidatedLirTargetSelection,
    pub(in crate::object_production) foundation: OdrFreeLirFoundation,
    pub(in crate::object_production) production: StrongProductionSectionV1,
    pub(in crate::object_production) c_bridge_profile: CBridgeToolchainProfileV1,
    pub(in crate::object_production) c_bridge_production: CBridgeProductionSetV1,
    pub(in crate::object_production) member_plan: PlannedLinkObjectMemberSetV1,
    pub(in crate::object_production) scoop_lir_members: Vec<PlannedScoopLirObjectInputV1>,
    pub(in crate::object_production) generated_c_bridge_members:
        Vec<PlannedGeneratedCBridgeObjectInputV1>,
    pub(in crate::object_production) digest_patches: Vec<ProvisionalDigestPatchSiteV1>,
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
            target_selection: scoop_lir.target_selection(),
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
        self.target_selection.target()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
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

    pub(super) fn c_bridge_candidates(&self) -> Vec<GeneratedCBridgeObjectCandidateV1<'_>> {
        self.generated_c_bridge_members
            .iter()
            .map(|member| {
                GeneratedCBridgeObjectCandidateV1::new(member.plan.member_id(), &member.bytes)
            })
            .collect()
    }

    pub(super) fn scoop_lir_candidates(&self) -> Vec<ScoopLirObjectCandidateV1<'_>> {
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
