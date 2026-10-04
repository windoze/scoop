//! Canonical member bindings for Scoop and generated-C objects.

use super::*;

mod codegen;
pub(super) use codegen::plan_codegen_objects;
mod finalization;
pub(super) use finalization::final_members;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinObjectProducerV1 {
    ScoopLir,
    GeneratedCBridge,
}

#[derive(Clone, Debug)]
pub(super) struct UnboundScoopLirObject {
    pub(in crate::object_production) units: Vec<ObjectDefinitionPlanId>,
    pub(in crate::object_production) bytes: Vec<u8>,
    pub(in crate::object_production) digest_patches: Vec<UnboundDigestPatch>,
}

#[derive(Clone, Debug)]
pub(super) struct UnboundGeneratedCBridgeObject {
    pub(in crate::object_production) unit: GeneratedBridgeUnitId,
    pub(in crate::object_production) bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct UnboundDigestPatch {
    pub(in crate::object_production) intent: scoop_identity::DigestPatchIntentId,
    pub(in crate::object_production) definition: ObjectDefinitionPlanId,
    pub(in crate::object_production) checked_object_offset: u64,
    pub(in crate::object_production) width_bytes: u8,
}

impl UnboundDigestPatch {
    pub(super) fn from_codegen(
        location: ProvisionalStrongDigestPatchLocationV1,
        offset: u64,
    ) -> Self {
        Self {
            intent: location.intent(),
            definition: location.definition(),
            checked_object_offset: offset,
            width_bytes: location.width_bytes(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PlannedObjectBindings {
    pub(in crate::object_production) member_plan: PlannedLinkObjectMemberSetV1,
    pub(in crate::object_production) scoop_lir_members: Vec<PlannedScoopLirObjectInputV1>,
    pub(in crate::object_production) generated_c_bridge_members:
        Vec<PlannedGeneratedCBridgeObjectInputV1>,
    pub(in crate::object_production) digest_patches: Vec<ProvisionalDigestPatchSiteV1>,
}

pub(super) fn plan_objects(
    target: LirTargetProfile,
    producer_units: &ProducerUnitPartitionV1,
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
        target,
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

pub(super) fn scoop_member_for_units<'plan>(
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

pub(super) fn generated_bridge_member_for_unit(
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
