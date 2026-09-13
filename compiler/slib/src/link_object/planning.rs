//! Complete provisional member assignment before object verification.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{ConeIdentity, GeneratedBridgeUnitId, ObjectDefinitionPlanId};
use scoop_lir::StrongProducerUnitPartitionV1;

use super::{
    CanonicalGeneratedBridgeObjectUnitSetV1, CanonicalScoopLirObjectUnitSetV1,
    LinkObjectMemberPlanError, PlannedGeneratedBridgeObjectMemberV1, PlannedScoopLirObjectMemberV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionPlanMemberAssignmentV1 {
    definition_plan: ObjectDefinitionPlanId,
    member: SlibMemberId,
}

impl DefinitionPlanMemberAssignmentV1 {
    pub const fn definition_plan(self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn member(self) -> SlibMemberId {
        self.member
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedBridgeUnitMemberAssignmentV1 {
    unit: GeneratedBridgeUnitId,
    member: SlibMemberId,
}

impl GeneratedBridgeUnitMemberAssignmentV1 {
    pub const fn unit(self) -> GeneratedBridgeUnitId {
        self.unit
    }

    pub const fn member(self) -> SlibMemberId {
        self.member
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedLinkObjectMemberSetV1 {
    producer: ConeIdentity,
    scoop_lir_members: Vec<PlannedScoopLirObjectMemberV1>,
    generated_bridge_members: Vec<PlannedGeneratedBridgeObjectMemberV1>,
    definition_assignments: Vec<DefinitionPlanMemberAssignmentV1>,
    generated_bridge_unit_assignments: Vec<GeneratedBridgeUnitMemberAssignmentV1>,
}

impl PlannedLinkObjectMemberSetV1 {
    pub fn new(
        cone: ConeIdentity,
        partition: &StrongProducerUnitPartitionV1,
        scoop_lir_unit_sets: Vec<CanonicalScoopLirObjectUnitSetV1>,
        generated_bridge_unit_sets: Vec<CanonicalGeneratedBridgeObjectUnitSetV1>,
    ) -> Result<Self, LinkObjectMemberSetPlanError> {
        if partition.scoop_lir_definition_plans().is_empty() {
            return Err(LinkObjectMemberSetPlanError::NoScoopLirDefinitions);
        }

        let mut scoop_lir_members = scoop_lir_unit_sets
            .into_iter()
            .map(|units| PlannedScoopLirObjectMemberV1::new(cone, units))
            .collect::<Result<Vec<_>, _>>()
            .map_err(LinkObjectMemberSetPlanError::Member)?;
        scoop_lir_members.sort_unstable_by_key(PlannedScoopLirObjectMemberV1::member_id);

        let expected_lir = partition
            .scoop_lir_definition_plans()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut definition_assignments = BTreeMap::new();
        for member in &scoop_lir_members {
            for plan in member.units().units() {
                if !expected_lir.contains(plan) {
                    return Err(LinkObjectMemberSetPlanError::UnexpectedScoopLirDefinition(
                        *plan,
                    ));
                }
                if definition_assignments
                    .insert(*plan, member.member_id())
                    .is_some()
                {
                    return Err(LinkObjectMemberSetPlanError::DuplicateScoopLirDefinition(
                        *plan,
                    ));
                }
            }
        }
        if let Some(plan) = expected_lir
            .iter()
            .find(|plan| !definition_assignments.contains_key(plan))
        {
            return Err(LinkObjectMemberSetPlanError::MissingScoopLirDefinition(
                *plan,
            ));
        }

        let mut generated_bridge_members = generated_bridge_unit_sets
            .into_iter()
            .map(|units| PlannedGeneratedBridgeObjectMemberV1::new(cone, units))
            .collect::<Result<Vec<_>, _>>()
            .map_err(LinkObjectMemberSetPlanError::Member)?;
        generated_bridge_members
            .sort_unstable_by_key(PlannedGeneratedBridgeObjectMemberV1::member_id);

        let expected_bridge_units = partition
            .generated_bridge_units()
            .iter()
            .map(|unit| unit.unit())
            .collect::<BTreeSet<_>>();
        let mut bridge_unit_assignments = BTreeMap::new();
        for member in &generated_bridge_members {
            for unit in member.units().units() {
                if !expected_bridge_units.contains(unit) {
                    return Err(LinkObjectMemberSetPlanError::UnexpectedGeneratedBridgeUnit(
                        *unit,
                    ));
                }
                if bridge_unit_assignments
                    .insert(*unit, member.member_id())
                    .is_some()
                {
                    return Err(LinkObjectMemberSetPlanError::DuplicateGeneratedBridgeUnit(
                        *unit,
                    ));
                }
            }
        }
        if let Some(unit) = expected_bridge_units
            .iter()
            .find(|unit| !bridge_unit_assignments.contains_key(unit))
        {
            return Err(LinkObjectMemberSetPlanError::MissingGeneratedBridgeUnit(
                *unit,
            ));
        }

        for unit in partition.generated_bridge_units() {
            let member = bridge_unit_assignments[&unit.unit()];
            for plan in unit.definition_plans() {
                if definition_assignments.insert(*plan, member).is_some() {
                    return Err(LinkObjectMemberSetPlanError::DuplicateDefinitionAssignment(
                        *plan,
                    ));
                }
            }
        }
        if definition_assignments.len() != partition.definition_plan_count() {
            return Err(LinkObjectMemberSetPlanError::DefinitionCoverageMismatch {
                expected: partition.definition_plan_count(),
                actual: definition_assignments.len(),
            });
        }

        let mut member_ids = BTreeSet::new();
        for member in &scoop_lir_members {
            if !member_ids.insert(member.member_id()) {
                return Err(LinkObjectMemberSetPlanError::DuplicateMemberIdentity(
                    member.member_id(),
                ));
            }
        }
        for member in &generated_bridge_members {
            if !member_ids.insert(member.member_id()) {
                return Err(LinkObjectMemberSetPlanError::DuplicateMemberIdentity(
                    member.member_id(),
                ));
            }
        }

        Ok(Self {
            producer: cone,
            scoop_lir_members,
            generated_bridge_members,
            definition_assignments: definition_assignments
                .into_iter()
                .map(
                    |(definition_plan, member)| DefinitionPlanMemberAssignmentV1 {
                        definition_plan,
                        member,
                    },
                )
                .collect(),
            generated_bridge_unit_assignments: bridge_unit_assignments
                .into_iter()
                .map(|(unit, member)| GeneratedBridgeUnitMemberAssignmentV1 { unit, member })
                .collect(),
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn scoop_lir_members(&self) -> &[PlannedScoopLirObjectMemberV1] {
        &self.scoop_lir_members
    }

    pub fn generated_bridge_members(&self) -> &[PlannedGeneratedBridgeObjectMemberV1] {
        &self.generated_bridge_members
    }

    pub fn definition_assignments(&self) -> &[DefinitionPlanMemberAssignmentV1] {
        &self.definition_assignments
    }

    pub fn generated_bridge_unit_assignments(&self) -> &[GeneratedBridgeUnitMemberAssignmentV1] {
        &self.generated_bridge_unit_assignments
    }

    pub fn member_for_definition(&self, plan: ObjectDefinitionPlanId) -> Option<SlibMemberId> {
        self.definition_assignments
            .binary_search_by_key(&plan, |assignment| assignment.definition_plan)
            .ok()
            .map(|index| self.definition_assignments[index].member)
    }

    pub fn member_for_generated_bridge_unit(
        &self,
        unit: GeneratedBridgeUnitId,
    ) -> Option<SlibMemberId> {
        self.generated_bridge_unit_assignments
            .binary_search_by_key(&unit, |assignment| assignment.unit)
            .ok()
            .map(|index| self.generated_bridge_unit_assignments[index].member)
    }
}

#[derive(Debug)]
pub enum LinkObjectMemberSetPlanError {
    NoScoopLirDefinitions,
    Member(LinkObjectMemberPlanError),
    UnexpectedScoopLirDefinition(ObjectDefinitionPlanId),
    DuplicateScoopLirDefinition(ObjectDefinitionPlanId),
    MissingScoopLirDefinition(ObjectDefinitionPlanId),
    UnexpectedGeneratedBridgeUnit(GeneratedBridgeUnitId),
    DuplicateGeneratedBridgeUnit(GeneratedBridgeUnitId),
    MissingGeneratedBridgeUnit(GeneratedBridgeUnitId),
    DuplicateDefinitionAssignment(ObjectDefinitionPlanId),
    DefinitionCoverageMismatch { expected: usize, actual: usize },
    DuplicateMemberIdentity(SlibMemberId),
}

impl fmt::Display for LinkObjectMemberSetPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid link-object member set plan: {self:?}")
    }
}

impl std::error::Error for LinkObjectMemberSetPlanError {}

#[cfg(test)]
mod tests;
