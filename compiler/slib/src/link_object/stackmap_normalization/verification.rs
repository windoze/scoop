//! Complete member-aware stackmap coverage against final LIR semantics.

use scoop_identity::{
    DefinitionAtomRole, ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentCallableBodyId,
    StrongDefinitionEntity, StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_lir::{StrongSafepointSemanticPlanSetV1, StrongSafepointSemanticPlanV1};
use std::collections::{BTreeMap, BTreeSet};

use super::{
    ProvisionalLlvmStackmapRecordHeaderV3, ProvisionalLlvmStackmapRecordV3,
    VerifiedDarwinArm64StackmapSectionV3, VerifiedNormalizedStackmapRecordV1,
    normalize_darwin_aarch64_stackmap_record_v1, verify_darwin_arm64_stackmap_section_v3,
};
use crate::SlibMemberId;
use crate::link_object::{
    PlannedStrongObjectSymbolRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedMemberObjectRelocationIndexV1,
};

mod aarch64;
pub use aarch64::DarwinAarch64StackmapMachineCodeError;
use aarch64::validate_stackmap_function_machine_code;

mod error;
pub use error::ScoopLirStackmapValidationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedScoopLirStackmapRecordV1 {
    member: SlibMemberId,
    function_symbol_table_index: u32,
    object_return_pc: u64,
    normalized: VerifiedNormalizedStackmapRecordV1,
}

impl VerifiedScoopLirStackmapRecordV1 {
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
pub struct VerifiedScoopLirStackmapSetV1 {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    semantic_plan: StrongSafepointSemanticPlanSetV1,
    records: Vec<VerifiedScoopLirStackmapRecordV1>,
}

impl VerifiedScoopLirStackmapSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.semantic_plan.producer()
    }

    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }

    pub const fn semantic_plan(&self) -> &StrongSafepointSemanticPlanSetV1 {
        &self.semantic_plan
    }

    pub fn records(&self) -> &[VerifiedScoopLirStackmapRecordV1] {
        &self.records
    }
}

pub fn verify_scoop_lir_stackmaps_v1(
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    semantic_plan: StrongSafepointSemanticPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedScoopLirStackmapSetV1, ScoopLirStackmapValidationError> {
    if builtins.producer() != semantic_plan.producer() {
        return Err(ScoopLirStackmapValidationError::ProducerMismatch {
            object: builtins.producer(),
            semantic: semantic_plan.producer(),
        });
    }
    validate_object_coverage(&builtins, scoop_objects)?;
    let expected_by_member = expected_sites_by_member(&builtins, &semantic_plan)?;
    let mut records = Vec::with_capacity(semantic_plan.sites().len());
    for object in scoop_objects {
        let member = verified_member(&builtins, object.member())?;
        let section = verify_darwin_arm64_stackmap_section_v3(
            object.bytes(),
            member.definitions().sections(),
        )
        .map_err(|source| ScoopLirStackmapValidationError::PhysicalSection {
            member: object.member(),
            source,
        })?;
        let expected = expected_by_member
            .get(&object.member())
            .map(Vec::as_slice)
            .unwrap_or_default();
        records.extend(verify_member_records(
            object.bytes(),
            member,
            section,
            expected,
        )?);
    }
    records.sort_unstable_by_key(|record| record.normalized.canonical().site());
    if let Some(pair) = records
        .windows(2)
        .find(|pair| pair[0].normalized.canonical().site() == pair[1].normalized.canonical().site())
    {
        return Err(ScoopLirStackmapValidationError::DuplicateSite(
            pair[0].normalized.canonical().site(),
        ));
    }
    if records.len() != semantic_plan.sites().len() {
        return Err(ScoopLirStackmapValidationError::GlobalRecordCoverage {
            expected: semantic_plan.sites().len(),
            actual: records.len(),
        });
    }
    Ok(VerifiedScoopLirStackmapSetV1 {
        builtins,
        semantic_plan,
        records,
    })
}

fn validate_object_coverage(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<(), ScoopLirStackmapValidationError> {
    for (index, pair) in objects.windows(2).enumerate() {
        if pair[0].member() >= pair[1].member() {
            return Err(if pair[0].member() == pair[1].member() {
                ScoopLirStackmapValidationError::DuplicateObjectMember(pair[0].member())
            } else {
                ScoopLirStackmapValidationError::NonCanonicalObjectOrder { index: index + 1 }
            });
        }
    }
    let expected = builtins
        .member_plan()
        .scoop_lir_members()
        .iter()
        .map(|member| member.member_id())
        .collect::<BTreeSet<_>>();
    let actual = objects
        .iter()
        .map(|object| object.member())
        .collect::<BTreeSet<_>>();
    if let Some(member) = actual.difference(&expected).next() {
        return Err(ScoopLirStackmapValidationError::UnexpectedObjectMember(
            *member,
        ));
    }
    if let Some(member) = expected.difference(&actual).next() {
        return Err(ScoopLirStackmapValidationError::MissingObjectMember(
            *member,
        ));
    }
    Ok(())
}

fn expected_sites_by_member(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    semantic_plan: &StrongSafepointSemanticPlanSetV1,
) -> Result<
    BTreeMap<SlibMemberId, Vec<StrongSafepointSemanticPlanV1>>,
    ScoopLirStackmapValidationError,
> {
    let scoop_members = builtins
        .member_plan()
        .scoop_lir_members()
        .iter()
        .map(|member| member.member_id())
        .collect::<BTreeSet<_>>();
    let mut by_member = BTreeMap::<SlibMemberId, Vec<_>>::new();
    for site in semantic_plan.sites() {
        let definition = callable_definition(semantic_plan.producer(), site.owner())?;
        let member = builtins
            .member_plan()
            .member_for_definition(definition)
            .ok_or(
                ScoopLirStackmapValidationError::MissingOwnerDefinitionAssignment {
                    site: site.site(),
                    definition,
                },
            )?;
        if !scoop_members.contains(&member) {
            return Err(
                ScoopLirStackmapValidationError::OwnerAssignedToNonScoopMember {
                    site: site.site(),
                    member,
                },
            );
        }
        by_member.entry(member).or_default().push(*site);
    }
    Ok(by_member)
}

fn callable_definition(
    producer: scoop_identity::ConeIdentity,
    owner: PersistentCallableBodyId,
) -> Result<ObjectDefinitionPlanId, ScoopLirStackmapValidationError> {
    let key = ObjectDefinitionPlanKey::strong(
        producer,
        StrongDefinitionEntity::callable_body(owner),
        StrongDefinitionRole::CallableBody,
    )
    .map_err(|_| ScoopLirStackmapValidationError::InvalidCallableDefinition(owner))?;
    ObjectDefinitionPlanId::from_key(&key)
        .map_err(ScoopLirStackmapValidationError::DefinitionIdentity)
}

fn verified_member(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    member: SlibMemberId,
) -> Result<&VerifiedMemberObjectRelocationIndexV1, ScoopLirStackmapValidationError> {
    builtins
        .strong_relocations()
        .members()
        .binary_search_by_key(&member, |item| item.member())
        .ok()
        .map(|index| &builtins.strong_relocations().members()[index])
        .ok_or(ScoopLirStackmapValidationError::MissingVerifiedMember(
            member,
        ))
}

fn verify_member_records(
    object_bytes: &[u8],
    member: &VerifiedMemberObjectRelocationIndexV1,
    section: Option<VerifiedDarwinArm64StackmapSectionV3>,
    expected: &[StrongSafepointSemanticPlanV1],
) -> Result<Vec<VerifiedScoopLirStackmapRecordV1>, ScoopLirStackmapValidationError> {
    let Some(section) = section else {
        return if expected.is_empty() {
            Ok(Vec::new())
        } else {
            Err(ScoopLirStackmapValidationError::MissingStackmapSection(
                member.member(),
            ))
        };
    };
    if expected.is_empty() {
        return Err(ScoopLirStackmapValidationError::UnexpectedStackmapSection(
            member.member(),
        ));
    }
    validate_stackmap_atom_roles(member, &section)?;
    if section.record_count() != expected.len() {
        return Err(ScoopLirStackmapValidationError::MemberRecordCoverage {
            member: member.member(),
            expected: expected.len(),
            actual: section.record_count(),
        });
    }

    let mut expected_by_owner = BTreeMap::<PersistentCallableBodyId, Vec<_>>::new();
    let mut expected_by_runtime_id = BTreeMap::new();
    for site in expected {
        expected_by_owner
            .entry(site.owner())
            .or_default()
            .push(*site);
        expected_by_runtime_id.insert(site.safepoint().get(), *site);
    }
    if section.functions().len() != expected_by_owner.len() {
        return Err(ScoopLirStackmapValidationError::MemberFunctionCoverage {
            member: member.member(),
            expected: expected_by_owner.len(),
            actual: section.functions().len(),
        });
    }

    let mut seen_owners = BTreeSet::new();
    let mut seen_sites = BTreeSet::new();
    let mut records = Vec::with_capacity(expected.len());
    for function in section.functions() {
        let symbol = member
            .definitions()
            .strong_symbol_by_table_index(function.target_symbol_table_index())
            .ok_or(ScoopLirStackmapValidationError::UnplannedFunctionTarget {
                member: member.member(),
                table_index: function.target_symbol_table_index(),
            })?;
        let PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition,
            owner,
            definition_role: StrongDefinitionRole::CallableBody,
            ..
        } = symbol.role()
        else {
            return Err(ScoopLirStackmapValidationError::NonCallableFunctionTarget {
                member: member.member(),
                table_index: function.target_symbol_table_index(),
            });
        };
        let StrongDefinitionEntityKind::CallableBody(owner) = owner.kind() else {
            return Err(ScoopLirStackmapValidationError::NonCallableFunctionTarget {
                member: member.member(),
                table_index: function.target_symbol_table_index(),
            });
        };
        if callable_definition(member.producer(), owner)? != definition {
            return Err(
                ScoopLirStackmapValidationError::CallableDefinitionMismatch {
                    member: member.member(),
                    owner,
                    definition,
                },
            );
        }
        let expected_for_owner = expected_by_owner.get(&owner).ok_or(
            ScoopLirStackmapValidationError::UnexpectedFunctionOwner {
                member: member.member(),
                owner,
            },
        )?;
        if !seen_owners.insert(owner) {
            return Err(ScoopLirStackmapValidationError::DuplicateFunctionOwner {
                member: member.member(),
                owner,
            });
        }
        if function.parsed().records().len() != expected_for_owner.len() {
            return Err(ScoopLirStackmapValidationError::FunctionRecordCoverage {
                member: member.member(),
                owner,
                expected: expected_for_owner.len(),
                actual: function.parsed().records().len(),
            });
        }
        let definition = member.definitions().definition(definition).ok_or(
            ScoopLirStackmapValidationError::MissingVerifiedCallableDefinition {
                member: member.member(),
                definition,
            },
        )?;
        let return_pcs = validate_stackmap_function_machine_code(
            object_bytes,
            member.definitions().sections(),
            definition,
            &symbol,
            function.parsed(),
        )
        .map_err(|source| ScoopLirStackmapValidationError::MachineCode {
            member: member.member(),
            owner,
            source,
        })?;
        for (parsed, object_return_pc) in function.parsed().records().iter().zip(return_pcs) {
            let plan = expected_by_runtime_id
                .get(&parsed.safepoint_id())
                .copied()
                .ok_or(ScoopLirStackmapValidationError::UnexpectedSafepointId {
                    member: member.member(),
                    owner,
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
            let normalized =
                normalize_darwin_aarch64_stackmap_record_v1(plan, section.constants(), provisional)
                    .map_err(|source| ScoopLirStackmapValidationError::Normalization {
                        member: member.member(),
                        safepoint_id: parsed.safepoint_id(),
                        source,
                    })?;
            if !seen_sites.insert(normalized.canonical().site()) {
                return Err(ScoopLirStackmapValidationError::DuplicateSite(
                    normalized.canonical().site(),
                ));
            }
            records.push(VerifiedScoopLirStackmapRecordV1 {
                member: member.member(),
                function_symbol_table_index: function.target_symbol_table_index(),
                object_return_pc,
                normalized,
            });
        }
    }
    if let Some(owner) = expected_by_owner
        .keys()
        .find(|owner| !seen_owners.contains(owner))
    {
        return Err(ScoopLirStackmapValidationError::MissingFunctionOwner {
            member: member.member(),
            owner: *owner,
        });
    }
    if let Some(site) = expected
        .iter()
        .map(|site| site.site())
        .find(|site| !seen_sites.contains(site))
    {
        return Err(ScoopLirStackmapValidationError::MissingSite {
            member: member.member(),
            site,
        });
    }
    Ok(records)
}

fn validate_stackmap_atom_roles(
    member: &VerifiedMemberObjectRelocationIndexV1,
    section: &VerifiedDarwinArm64StackmapSectionV3,
) -> Result<(), ScoopLirStackmapValidationError> {
    for atom in member
        .definitions()
        .definitions()
        .iter()
        .flat_map(|definition| definition.atoms())
        .filter(|atom| u32::from(atom.section_ordinal().get()) == section.section_ordinal().get())
    {
        if atom.atom_role() != DefinitionAtomRole::Stackmap {
            return Err(ScoopLirStackmapValidationError::NonStackmapAtomInSection {
                member: member.member(),
                atom: atom.atom(),
                actual: atom.atom_role(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
