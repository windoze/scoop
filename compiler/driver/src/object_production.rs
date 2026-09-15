//! Bind codegen-produced Scoop objects to canonical `.slib` member identities.

use std::fmt;
use std::path::PathBuf;

use scoop_codegen::{
    EmittedStrongObjectMemberKindV1, EmittedStrongObjectSetV1,
    ProvisionalStrongDigestPatchLocationV1,
};
use scoop_lir::{ObjectDefinitionPlanId, StrongProducerUnitPartitionV1, StrongProductionSectionV1};
use scoop_slib::{
    CanonicalGeneratedBridgeObjectUnitSetV1, CanonicalScoopLirObjectUnitSetV1,
    LinkObjectMemberSetPlanError, ObjectUnitSetError, PlannedLinkObjectMemberSetV1,
    PlannedScoopLirObjectMemberV1, ProvisionalDigestPatchSiteV1, ScoopLirObjectCandidateV1,
    SlibMemberId,
};

/// Immutable bytes for one codegen member after its stable `.slib` identity
/// has been derived from the exact producer unit set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedScoopLirObjectInputV1 {
    plan: PlannedScoopLirObjectMemberV1,
    bytes: Vec<u8>,
}

impl PlannedScoopLirObjectInputV1 {
    pub const fn plan(&self) -> &PlannedScoopLirObjectMemberV1 {
        &self.plan
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Complete codegen object set after member planning and typed digest-site
/// member binding, ready for the `.slib` object verifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedScoopLirObjectProductionV1 {
    production: StrongProductionSectionV1,
    member_plan: PlannedLinkObjectMemberSetV1,
    members: Vec<PlannedScoopLirObjectInputV1>,
    digest_patches: Vec<ProvisionalDigestPatchSiteV1>,
}

impl PlannedScoopLirObjectProductionV1 {
    pub fn from_codegen(
        emitted: &EmittedStrongObjectSetV1,
        generated_bridge_unit_sets: Vec<CanonicalGeneratedBridgeObjectUnitSetV1>,
    ) -> Result<Self, ScoopLirObjectProductionError> {
        let mut sources = Vec::with_capacity(emitted.members().len());
        for member in emitted.members() {
            let bytes = std::fs::read(member.path()).map_err(|source| {
                ScoopLirObjectProductionError::ReadObject {
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
            sources.push(UnboundScoopLirObject {
                units: member.units().definition_plans().to_vec(),
                bytes,
                digest_patches,
            });
        }
        let bindings = plan_objects(
            emitted.partition().producer_units(),
            sources,
            generated_bridge_unit_sets,
        )?;
        if bindings.digest_patches.is_empty() {
            return Err(ScoopLirObjectProductionError::EmptyDigestMaterializationSet);
        }
        Ok(Self {
            production: emitted.production().clone(),
            member_plan: bindings.member_plan,
            members: bindings.members,
            digest_patches: bindings.digest_patches,
        })
    }

    pub const fn production(&self) -> &StrongProductionSectionV1 {
        &self.production
    }

    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    pub fn members(&self) -> &[PlannedScoopLirObjectInputV1] {
        &self.members
    }

    pub fn digest_patches(&self) -> &[ProvisionalDigestPatchSiteV1] {
        &self.digest_patches
    }

    pub fn candidates(&self) -> Vec<ScoopLirObjectCandidateV1<'_>> {
        self.members
            .iter()
            .map(|member| ScoopLirObjectCandidateV1::new(member.plan.member_id(), &member.bytes))
            .collect()
    }
}

#[derive(Clone, Debug)]
struct UnboundScoopLirObject {
    units: Vec<ObjectDefinitionPlanId>,
    bytes: Vec<u8>,
    digest_patches: Vec<UnboundDigestPatch>,
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
    members: Vec<PlannedScoopLirObjectInputV1>,
    digest_patches: Vec<ProvisionalDigestPatchSiteV1>,
}

fn plan_objects(
    producer_units: &StrongProducerUnitPartitionV1,
    sources: Vec<UnboundScoopLirObject>,
    generated_bridge_unit_sets: Vec<CanonicalGeneratedBridgeObjectUnitSetV1>,
) -> Result<PlannedObjectBindings, ScoopLirObjectProductionError> {
    let unit_sets = sources
        .iter()
        .map(|source| CanonicalScoopLirObjectUnitSetV1::new(source.units.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ScoopLirObjectProductionError::Units)?;
    let member_plan =
        PlannedLinkObjectMemberSetV1::new(producer_units, unit_sets, generated_bridge_unit_sets)
            .map_err(ScoopLirObjectProductionError::MemberPlan)?;

    let mut members = Vec::with_capacity(sources.len());
    let mut digest_patches = Vec::new();
    for source in sources {
        let member = member_for_units(&member_plan, &source.units)?;
        for patch in source.digest_patches {
            let assigned = member_plan.member_for_definition(patch.definition).ok_or(
                ScoopLirObjectProductionError::UnassignedPatchDefinition(patch.definition),
            )?;
            if assigned != member.member_id() {
                return Err(ScoopLirObjectProductionError::PatchMemberMismatch {
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
        members.push(PlannedScoopLirObjectInputV1 {
            plan: member.clone(),
            bytes: source.bytes,
        });
    }
    members.sort_unstable_by_key(|member| member.plan.member_id());
    digest_patches.sort_unstable_by_key(|patch| patch.intent());
    if let Some(pair) = digest_patches
        .windows(2)
        .find(|pair| pair[0].intent() == pair[1].intent())
    {
        return Err(ScoopLirObjectProductionError::DuplicateDigestIntent(
            pair[0].intent(),
        ));
    }
    Ok(PlannedObjectBindings {
        member_plan,
        members,
        digest_patches,
    })
}

fn member_for_units<'plan>(
    member_plan: &'plan PlannedLinkObjectMemberSetV1,
    units: &[ObjectDefinitionPlanId],
) -> Result<&'plan PlannedScoopLirObjectMemberV1, ScoopLirObjectProductionError> {
    let first = *units
        .first()
        .ok_or(ScoopLirObjectProductionError::EmptyObjectUnits)?;
    let member = member_plan.member_for_definition(first).ok_or(
        ScoopLirObjectProductionError::UnassignedObjectDefinition(first),
    )?;
    if units
        .iter()
        .any(|definition| member_plan.member_for_definition(*definition) != Some(member))
    {
        return Err(ScoopLirObjectProductionError::SplitObjectUnits(member));
    }
    member_plan
        .scoop_lir_members()
        .iter()
        .find(|plan| plan.member_id() == member)
        .ok_or(ScoopLirObjectProductionError::MissingPlannedMember(member))
}

#[derive(Debug)]
pub enum ScoopLirObjectProductionError {
    ReadObject {
        path: PathBuf,
        source: std::io::Error,
    },
    Units(ObjectUnitSetError),
    MemberPlan(LinkObjectMemberSetPlanError),
    EmptyObjectUnits,
    UnassignedObjectDefinition(ObjectDefinitionPlanId),
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

impl fmt::Display for ScoopLirObjectProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Scoop LIR object production: {self:?}")
    }
}

impl std::error::Error for ScoopLirObjectProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadObject { source, .. } => Some(source),
            Self::Units(source) => Some(source),
            Self::MemberPlan(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
