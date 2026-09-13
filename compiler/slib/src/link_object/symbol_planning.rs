//! Member-aware expected strong symbols for provisional object verification.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    ConeIdentity, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{LirTargetProfile, StrongObjectSymbolSurfaceV1};

use super::PlannedLinkObjectMemberSetV1;
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlannedStrongObjectSymbolRoleV1 {
    PrimaryDefinition {
        definition: ObjectDefinitionPlanId,
        owner: StrongDefinitionEntity,
        definition_role: StrongDefinitionRole,
        primary_atom: ObjectDefinitionAtomId,
    },
    AtomBoundaryStart {
        definition: ObjectDefinitionPlanId,
        atom: ObjectDefinitionAtomId,
    },
    AtomBoundaryEnd {
        definition: ObjectDefinitionPlanId,
        atom: ObjectDefinitionAtomId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedStrongObjectSymbolV1 {
    role: PlannedStrongObjectSymbolRoleV1,
    request: PersistentSymbolRequest,
    macho_name: Vec<u8>,
}

impl PlannedStrongObjectSymbolV1 {
    pub const fn role(&self) -> PlannedStrongObjectSymbolRoleV1 {
        self.role
    }

    pub const fn request(&self) -> PersistentSymbolRequest {
        self.request
    }

    pub fn macho_name(&self) -> &[u8] {
        &self.macho_name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMemberStrongObjectSymbolsV1 {
    producer: ConeIdentity,
    member: SlibMemberId,
    symbols: Vec<PlannedStrongObjectSymbolV1>,
}

impl PlannedMemberStrongObjectSymbolsV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub fn symbols(&self) -> &[PlannedStrongObjectSymbolV1] {
        &self.symbols
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedStrongObjectSymbolSetV1 {
    producer: ConeIdentity,
    members: Vec<PlannedMemberStrongObjectSymbolsV1>,
}

impl PlannedStrongObjectSymbolSetV1 {
    pub fn new(
        target: LirTargetProfile,
        surface: &StrongObjectSymbolSurfaceV1,
        member_plan: &PlannedLinkObjectMemberSetV1,
    ) -> Result<Self, StrongObjectSymbolPlanningError> {
        let normalization = target.contract().native_symbol_normalization();
        let expected_plans = member_plan
            .definition_assignments()
            .iter()
            .map(|assignment| assignment.definition_plan())
            .collect::<BTreeSet<_>>();
        let actual_plans = surface
            .plans()
            .iter()
            .map(|plan| plan.definition_plan())
            .collect::<BTreeSet<_>>();
        if let Some(plan) = actual_plans.difference(&expected_plans).next() {
            return Err(StrongObjectSymbolPlanningError::MissingMemberAssignment(
                *plan,
            ));
        }
        if let Some(plan) = expected_plans.difference(&actual_plans).next() {
            return Err(StrongObjectSymbolPlanningError::MissingSymbolPlan(*plan));
        }

        let mut by_member = BTreeMap::<SlibMemberId, BTreeMap<Vec<u8>, _>>::new();
        for plan in surface.plans() {
            let member = member_plan
                .member_for_definition(plan.definition_plan())
                .ok_or(StrongObjectSymbolPlanningError::MissingMemberAssignment(
                    plan.definition_plan(),
                ))?;
            insert_symbol(
                &mut by_member,
                member,
                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                    definition: plan.definition_plan(),
                    owner: plan.owner(),
                    definition_role: plan.definition_role(),
                    primary_atom: plan.primary_atom(),
                },
                plan.primary_symbol(),
                normalization,
            )?;
            for boundary in plan.atom_boundaries() {
                insert_symbol(
                    &mut by_member,
                    member,
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
                        definition: plan.definition_plan(),
                        atom: boundary.atom(),
                    },
                    boundary.start(),
                    normalization,
                )?;
                insert_symbol(
                    &mut by_member,
                    member,
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
                        definition: plan.definition_plan(),
                        atom: boundary.atom(),
                    },
                    boundary.end(),
                    normalization,
                )?;
            }
        }

        let expected_members = member_plan
            .scoop_lir_members()
            .iter()
            .map(|member| member.member_id())
            .chain(
                member_plan
                    .generated_bridge_members()
                    .iter()
                    .map(|member| member.member_id()),
            )
            .collect::<BTreeSet<_>>();
        if let Some(member) = expected_members
            .iter()
            .find(|member| !by_member.contains_key(member))
        {
            return Err(StrongObjectSymbolPlanningError::EmptyMember(*member));
        }
        if by_member.len() != expected_members.len() {
            return Err(StrongObjectSymbolPlanningError::MemberCoverageMismatch {
                expected: expected_members.len(),
                actual: by_member.len(),
            });
        }

        Ok(Self {
            producer: member_plan.producer(),
            members: by_member
                .into_iter()
                .map(|(member, symbols)| PlannedMemberStrongObjectSymbolsV1 {
                    producer: member_plan.producer(),
                    member,
                    symbols: symbols.into_values().collect(),
                })
                .collect(),
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn members(&self) -> &[PlannedMemberStrongObjectSymbolsV1] {
        &self.members
    }

    pub fn member(&self, member: SlibMemberId) -> Option<&PlannedMemberStrongObjectSymbolsV1> {
        self.members
            .binary_search_by_key(&member, |symbols| symbols.member)
            .ok()
            .map(|index| &self.members[index])
    }
}

fn insert_symbol(
    by_member: &mut BTreeMap<SlibMemberId, BTreeMap<Vec<u8>, PlannedStrongObjectSymbolV1>>,
    member: SlibMemberId,
    role: PlannedStrongObjectSymbolRoleV1,
    request: PersistentSymbolRequest,
    normalization: scoop_lir::NativeSymbolNormalization,
) -> Result<(), StrongObjectSymbolPlanningError> {
    let macho_name = normalization
        .compiler_generated_object_symbol(request.symbol().as_str())
        .into_bytes();
    let symbol = PlannedStrongObjectSymbolV1 {
        role,
        request,
        macho_name: macho_name.clone(),
    };
    if by_member
        .entry(member)
        .or_default()
        .insert(macho_name.clone(), symbol)
        .is_some()
    {
        return Err(StrongObjectSymbolPlanningError::DuplicateMachOSymbol { member, macho_name });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongObjectSymbolPlanningError {
    MissingMemberAssignment(ObjectDefinitionPlanId),
    MissingSymbolPlan(ObjectDefinitionPlanId),
    DuplicateMachOSymbol {
        member: SlibMemberId,
        macho_name: Vec<u8>,
    },
    EmptyMember(SlibMemberId),
    MemberCoverageMismatch {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for StrongObjectSymbolPlanningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong object symbol plan: {self:?}")
    }
}

impl std::error::Error for StrongObjectSymbolPlanningError {}

#[cfg(test)]
mod tests;
