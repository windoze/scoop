//! Read emitted stackmaps and join them to their callable and safepoint identities.

use scoop_identity::{StrongDefinitionEntityKind, StrongDefinitionRole};
use scoop_lir::{StrongSafepointSemanticPlanSetV1, StrongSafepointSemanticPlanV1};
use std::collections::BTreeMap;

use super::{
    ObjectStackmapSectionV3, ProvisionalLlvmStackmapRecordHeaderV3,
    ProvisionalLlvmStackmapRecordV3, VerifiedNormalizedStackmapRecordV1,
    normalize_stackmap_record_v1, read_object_stackmap_section_v3,
};
use crate::SlibMemberId;
use crate::link_object::{
    PlannedStrongObjectSymbolRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedMemberObjectRelocationIndexV1,
};

mod error;
pub use error::ScoopStackmapReadError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopStackmapRecordV1 {
    member: SlibMemberId,
    function_symbol_table_index: u32,
    object_return_pc: u64,
    normalized: VerifiedNormalizedStackmapRecordV1,
}

impl ScoopStackmapRecordV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn function_symbol_table_index(&self) -> u32 {
        self.function_symbol_table_index
    }

    /// Original section-relative return PC in the relocatable object.
    pub const fn object_return_pc(&self) -> u64 {
        self.object_return_pc
    }

    pub const fn normalized(&self) -> &VerifiedNormalizedStackmapRecordV1 {
        &self.normalized
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopStackmapSetV1 {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    semantic_plan: StrongSafepointSemanticPlanSetV1,
    records: Vec<ScoopStackmapRecordV1>,
}

impl ScoopStackmapSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.semantic_plan.producer()
    }

    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }

    pub const fn semantic_plan(&self) -> &StrongSafepointSemanticPlanSetV1 {
        &self.semantic_plan
    }

    pub fn records(&self) -> &[ScoopStackmapRecordV1] {
        &self.records
    }
}

pub fn read_scoop_stackmaps_v1(
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    semantic_plan: StrongSafepointSemanticPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<ScoopStackmapSetV1, ScoopStackmapReadError> {
    let sites = semantic_plan
        .sites()
        .iter()
        .map(|site| (site.safepoint().get(), *site))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::with_capacity(sites.len());
    for object in scoop_objects {
        let members = builtins.strong_relocations().members();
        let index = members
            .binary_search_by_key(&object.member(), |member| member.member())
            .map_err(|_| ScoopStackmapReadError::MissingMember(object.member()))?;
        let member = &members[index];
        if let Some(section) =
            read_object_stackmap_section_v3(object.bytes(), member.definitions().sections())
                .map_err(|source| ScoopStackmapReadError::PhysicalSection {
                    member: object.member(),
                    source,
                })?
        {
            read_member_records(member, section, &sites, &mut records)?;
        }
    }
    records.sort_unstable_by_key(|record| record.normalized.canonical().site());
    Ok(ScoopStackmapSetV1 {
        builtins,
        semantic_plan,
        records,
    })
}

fn read_member_records(
    member: &VerifiedMemberObjectRelocationIndexV1,
    section: ObjectStackmapSectionV3,
    sites: &BTreeMap<u64, StrongSafepointSemanticPlanV1>,
    records: &mut Vec<ScoopStackmapRecordV1>,
) -> Result<(), ScoopStackmapReadError> {
    for function in section.functions() {
        let symbol = member
            .definitions()
            .strong_symbol_by_table_index(function.target_symbol_table_index())
            .ok_or(ScoopStackmapReadError::UnplannedFunctionTarget {
                member: member.member(),
                table_index: function.target_symbol_table_index(),
            })?;
        let PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            owner,
            definition_role: StrongDefinitionRole::CallableBody,
            ..
        } = symbol.role()
        else {
            return Err(ScoopStackmapReadError::NonCallableFunctionTarget {
                member: member.member(),
                table_index: function.target_symbol_table_index(),
            });
        };
        let StrongDefinitionEntityKind::CallableBody(owner) = owner.kind() else {
            return Err(ScoopStackmapReadError::NonCallableFunctionTarget {
                member: member.member(),
                table_index: function.target_symbol_table_index(),
            });
        };
        for parsed in function.parsed().records() {
            let plan = sites.get(&parsed.safepoint_id()).copied().ok_or(
                ScoopStackmapReadError::UnexpectedSafepointId {
                    member: member.member(),
                    owner,
                    safepoint_id: parsed.safepoint_id(),
                },
            )?;
            let object_return_pc = symbol
                .value()
                .checked_add(u64::from(parsed.instruction_offset()))
                .ok_or(ScoopStackmapReadError::ReturnAddressOverflow {
                    member: member.member(),
                    safepoint_id: parsed.safepoint_id(),
                })?;
            let provisional = ProvisionalLlvmStackmapRecordV3::new(
                ProvisionalLlvmStackmapRecordHeaderV3::new(
                    parsed.safepoint_id(),
                    owner,
                    parsed.instruction_offset(),
                    function.parsed().stack_size(),
                    parsed.flags(),
                ),
                parsed.locations().to_vec(),
                parsed.live_outs().to_vec(),
            );
            let normalized = normalize_stackmap_record_v1(
                member.definitions().sections().envelope().target(),
                plan,
                section.constants(),
                provisional,
            )
            .map_err(|source| ScoopStackmapReadError::Normalization {
                member: member.member(),
                safepoint_id: parsed.safepoint_id(),
                source,
            })?;
            records.push(ScoopStackmapRecordV1 {
                member: member.member(),
                function_symbol_table_index: function.target_symbol_table_index(),
                object_return_pc,
                normalized,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
