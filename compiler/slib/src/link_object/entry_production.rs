//! Exact object verification for the library/executable entry branch.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestSemanticFieldRole,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_lir::{EntryProductionPlanV1, ExecutableEntryPlanV1};

use self::record::{ROOT_ENTRY_DESCRIPTOR_SIZE, expected_root_entry_record};
use super::safepoint_registrations::physical::{
    atom_file_range, validate_objects, verified_member,
};
use super::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    ScoopLirObjectCandidateV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    StrongSafepointRegistrationValidationError, VerifiedMaterializedPatchSiteV1,
    VerifiedScoopLirDigestPatchSiteSetV1,
};
use crate::SlibMemberId;

mod finalization;
mod record;
pub use finalization::*;

const SOURCE_SIGNATURE_OFFSET: u64 = 80;
const GATEWAY_DEFINITION_OFFSET: u64 = 144;
const FAILURE_ROOT_POINTER_OFFSET: u64 = 176;
const GATEWAY_POINTER_OFFSET: u64 = 184;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedExecutableEntryV1 {
    member: SlibMemberId,
    checked_offset: u64,
    source_signature_patch: VerifiedMaterializedPatchSiteV1,
    gateway_definition_patch: VerifiedMaterializedPatchSiteV1,
    failure_root_relocation: StrongRelocationBindingV1,
    gateway_relocation: StrongRelocationBindingV1,
}

impl VerifiedExecutableEntryV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn checked_offset(&self) -> u64 {
        self.checked_offset
    }

    pub const fn source_signature_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.source_signature_patch
    }

    pub const fn gateway_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.gateway_definition_patch
    }

    pub const fn failure_root_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.failure_root_relocation
    }

    pub const fn gateway_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.gateway_relocation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedEntryProductionBranchV1 {
    Library,
    Executable(Box<VerifiedExecutableEntryV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEntryProductionV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: EntryProductionPlanV1,
    branch: VerifiedEntryProductionBranchV1,
}

impl VerifiedEntryProductionV1 {
    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &EntryProductionPlanV1 {
        &self.plan
    }

    pub const fn branch(&self) -> &VerifiedEntryProductionBranchV1 {
        &self.branch
    }
}

pub fn verify_entry_production_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: EntryProductionPlanV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedEntryProductionV1, EntryProductionValidationError> {
    let objects = validate_objects(patch_sites.builtins(), scoop_objects)
        .map_err(EntryProductionValidationError::ObjectValidation)?;
    let root_definitions = root_definitions(&patch_sites);
    let branch = match &plan {
        EntryProductionPlanV1::Library => {
            if !root_definitions.is_empty() {
                return Err(EntryProductionValidationError::LibraryRootDefinitions(
                    root_definitions,
                ));
            }
            VerifiedEntryProductionBranchV1::Library
        }
        EntryProductionPlanV1::Executable(plan) => {
            if root_definitions.as_slice() != [plan.root_descriptor_definition()] {
                return Err(EntryProductionValidationError::ExecutableRootDefinitions {
                    expected: plan.root_descriptor_definition(),
                    actual: root_definitions,
                });
            }
            VerifiedEntryProductionBranchV1::Executable(Box::new(verify_executable(
                &patch_sites,
                &objects,
                plan,
            )?))
        }
    };
    Ok(VerifiedEntryProductionV1 {
        patch_sites,
        plan,
        branch,
    })
}

fn root_definitions(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
) -> Vec<ObjectDefinitionPlanId> {
    patch_sites
        .builtins()
        .strong_relocations()
        .members()
        .iter()
        .flat_map(|member| member.definitions().symbols())
        .filter_map(|symbol| {
            if let super::PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                definition,
                owner,
                definition_role: StrongDefinitionRole::RootEntryDescriptor,
                ..
            } = symbol.role()
                && matches!(owner.kind(), StrongDefinitionEntityKind::RootEntry(_))
            {
                Some(definition)
            } else {
                None
            }
        })
        .collect()
}

fn verify_executable(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &ExecutableEntryPlanV1,
) -> Result<VerifiedExecutableEntryV1, EntryProductionValidationError> {
    let member = required_scoop_member(patch_sites, plan.root_descriptor_definition())?;
    let verified_member = verified_member(patch_sites.builtins(), member)
        .map_err(EntryProductionValidationError::ObjectValidation)?;
    let definition = verified_member
        .definitions()
        .definition(plan.root_descriptor_definition())
        .ok_or(EntryProductionValidationError::MissingVerifiedDefinition(
            plan.root_descriptor_definition(),
        ))?;
    if definition.atoms().len() != 1
        || definition.atoms()[0].atom() != definition.primary_atom()
        || definition.atoms()[0].atom_role() != DefinitionAtomRole::Primary
    {
        return Err(EntryProductionValidationError::RootAtomSet);
    }
    let atom = definition.atoms()[0];
    let (section_role, start, end) = atom_file_range(verified_member, atom)
        .map_err(EntryProductionValidationError::AtomRange)?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(EntryProductionValidationError::RootSection(section_role));
    }
    if end - start != ROOT_ENTRY_DESCRIPTOR_SIZE as u64 {
        return Err(EntryProductionValidationError::RootSize(end - start));
    }
    if atom.start() % 8 != 0 {
        return Err(EntryProductionValidationError::RootAlignment(atom.start()));
    }
    validate_record(objects[&member], start, plan)?;

    let source_node = DigestNodeId::from_key(&DigestNodeKey::source_signature(plan.main().body()))
        .map_err(EntryProductionValidationError::Hash)?;
    let gateway_plan = object_definition_plan(
        patch_sites,
        StrongDefinitionEntity::callable_body(plan.gateway()),
        StrongDefinitionRole::CallableBody,
    )?;
    let gateway_definition = require_definition(patch_sites, gateway_plan)?;
    let gateway_node = DigestNodeId::from_key(&DigestNodeKey::object_definition(
        gateway_definition.primary_atom(),
    ))
    .map_err(EntryProductionValidationError::Hash)?;
    let source_signature_patch = require_patch(
        patch_sites,
        plan.root_descriptor_definition(),
        definition.primary_atom(),
        member,
        start,
        plan.source_signature_patch(),
        source_node,
        DigestSemanticFieldRole::SourceSignature,
        SOURCE_SIGNATURE_OFFSET,
    )?;
    let gateway_definition_patch = require_patch(
        patch_sites,
        plan.root_descriptor_definition(),
        definition.primary_atom(),
        member,
        start,
        plan.gateway_definition_patch(),
        gateway_node,
        DigestSemanticFieldRole::GatewayDefinition,
        GATEWAY_DEFINITION_OFFSET,
    )?;
    validate_exact_patch_set(patch_sites, definition.primary_atom(), plan)?;

    let failure_plan = object_definition_plan(
        patch_sites,
        StrongDefinitionEntity::static_storage(plan.failure_root()),
        StrongDefinitionRole::RootRegistration,
    )?;
    let mut bindings = patch_sites
        .builtins()
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == member && binding.containing_atom() == atom.atom()
        })
        .cloned()
        .collect::<Vec<_>>();
    bindings.sort_unstable_by_key(|binding| binding.offset_within_atom());
    let physical_count = verified_member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == atom.atom())
        .count();
    if bindings.len() != 2 || physical_count != 2 {
        return Err(EntryProductionValidationError::RelocationCount(
            physical_count,
        ));
    }
    let failure_root_relocation = require_relocation(
        patch_sites,
        &bindings[0],
        member,
        atom.atom(),
        FAILURE_ROOT_POINTER_OFFSET,
        failure_plan,
        StrongDefinitionEntity::static_storage(plan.failure_root()),
        StrongDefinitionRole::RootRegistration,
    )?;
    let gateway_relocation = require_relocation(
        patch_sites,
        &bindings[1],
        member,
        atom.atom(),
        GATEWAY_POINTER_OFFSET,
        gateway_plan,
        StrongDefinitionEntity::callable_body(plan.gateway()),
        StrongDefinitionRole::CallableBody,
    )?;

    Ok(VerifiedExecutableEntryV1 {
        member,
        checked_offset: start,
        source_signature_patch,
        gateway_definition_patch,
        failure_root_relocation,
        gateway_relocation,
    })
}

fn required_scoop_member(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    definition: ObjectDefinitionPlanId,
) -> Result<SlibMemberId, EntryProductionValidationError> {
    let member = patch_sites
        .builtins()
        .member_plan()
        .member_for_definition(definition)
        .ok_or(EntryProductionValidationError::MissingDefinitionAssignment(
            definition,
        ))?;
    patch_sites
        .builtins()
        .member_plan()
        .scoop_lir_members()
        .iter()
        .any(|candidate| candidate.member_id() == member)
        .then_some(member)
        .ok_or(EntryProductionValidationError::DefinitionAssignedToNonScoopMember(member))
}

fn object_definition_plan(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<ObjectDefinitionPlanId, EntryProductionValidationError> {
    let key = ObjectDefinitionPlanKey::strong(patch_sites.producer(), entity, role)
        .map_err(EntryProductionValidationError::DefinitionIdentity)?;
    let id =
        ObjectDefinitionPlanId::from_key(&key).map_err(EntryProductionValidationError::Hash)?;
    require_definition(patch_sites, id).map(|_| id)
}

fn require_definition(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    definition: ObjectDefinitionPlanId,
) -> Result<&super::VerifiedStrongObjectDefinitionV1, EntryProductionValidationError> {
    let member = required_scoop_member(patch_sites, definition)?;
    verified_member(patch_sites.builtins(), member)
        .map_err(EntryProductionValidationError::ObjectValidation)?
        .definitions()
        .definition(definition)
        .ok_or(EntryProductionValidationError::MissingVerifiedDefinition(
            definition,
        ))
}

#[allow(clippy::too_many_arguments)]
fn require_patch(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    definition: ObjectDefinitionPlanId,
    atom: scoop_identity::ObjectDefinitionAtomId,
    member: SlibMemberId,
    record_start: u64,
    intent: scoop_identity::DigestPatchIntentId,
    source: DigestNodeId,
    role: DigestSemanticFieldRole,
    offset: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, EntryProductionValidationError> {
    let site = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == intent)
        .copied()
        .ok_or(EntryProductionValidationError::MissingPatch(intent))?;
    let checked_offset = record_start
        .checked_add(offset)
        .ok_or(EntryProductionValidationError::RecordRange)?;
    if site.source() != source
        || site.semantic_field_role() != role
        || site.member() != member
        || site.definition() != definition
        || site.atom() != atom
        || site.atom_role() != DefinitionAtomRole::Primary
        || site.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
        || site.offset_within_atom() != offset
        || site.checked_offset() != checked_offset
        || site.width_bytes() != 32
    {
        return Err(EntryProductionValidationError::PatchMismatch(intent));
    }
    Ok(site)
}

fn validate_exact_patch_set(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    atom: scoop_identity::ObjectDefinitionAtomId,
    plan: &ExecutableEntryPlanV1,
) -> Result<(), EntryProductionValidationError> {
    let actual = patch_sites
        .sites()
        .iter()
        .filter(|site| site.atom() == atom)
        .map(|site| site.intent())
        .collect::<Vec<_>>();
    let mut expected = vec![
        plan.source_signature_patch(),
        plan.gateway_definition_patch(),
    ];
    expected.sort_unstable();
    if actual != expected {
        return Err(EntryProductionValidationError::PatchSetMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_relocation(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    binding: &StrongRelocationBindingV1,
    source_member: SlibMemberId,
    containing_atom: scoop_identity::ObjectDefinitionAtomId,
    offset: u64,
    target_plan: ObjectDefinitionPlanId,
    target_entity: StrongDefinitionEntity,
    target_role: StrongDefinitionRole,
) -> Result<StrongRelocationBindingV1, EntryProductionValidationError> {
    if binding.source_member() != source_member
        || binding.containing_atom() != containing_atom
        || binding.containing_atom_role() != DefinitionAtomRole::Primary
        || binding.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
        || binding.offset_within_atom() != offset
        || binding.width_bytes() != 8
        || binding
            .relocation_form()
            .absolute64_addend(binding.encoded_value())
            != Some(0)
        || binding.target_slot() != RelocationTargetSlotV1::Single
    {
        return Err(EntryProductionValidationError::RelocationShape(offset));
    }
    let target_member = required_scoop_member(patch_sites, target_plan)?;
    let target_definition = require_definition(patch_sites, target_plan)?;
    let target_verified_member = verified_member(patch_sites.builtins(), target_member)
        .map_err(EntryProductionValidationError::ObjectValidation)?;
    let target_symbol = target_verified_member
        .definitions()
        .strong_symbol_by_table_index(target_definition.primary_symbol_table_index())
        .ok_or(EntryProductionValidationError::MissingTargetSymbol(
            target_plan,
        ))?;
    let expected_owner = LinkDefinitionOwnerV1::from_strong_primary(target_entity, target_role)
        .map_err(EntryProductionValidationError::TargetOwner)?;
    let (actual_member, actual_definition, actual_owner) = match binding.resolution() {
        StrongRelocationResolutionV1::ObjectLocalStrong {
            target_member,
            definition,
            owner,
        }
        | StrongRelocationResolutionV1::CurrentConeUndefinedStrong {
            target_member,
            definition,
            owner,
        } => (target_member, definition, owner),
        StrongRelocationResolutionV1::ExternalCandidate { .. } => {
            return Err(EntryProductionValidationError::RelocationTarget(offset));
        }
    };
    if actual_member != target_member
        || actual_definition != target_plan
        || actual_owner != expected_owner
        || binding.symbol() != target_symbol.macho_name()
    {
        return Err(EntryProductionValidationError::RelocationTarget(offset));
    }
    Ok(binding.clone())
}

fn validate_record(
    object: &[u8],
    checked_offset: u64,
    plan: &ExecutableEntryPlanV1,
) -> Result<(), EntryProductionValidationError> {
    let start =
        usize::try_from(checked_offset).map_err(|_| EntryProductionValidationError::RecordRange)?;
    let end = start
        .checked_add(ROOT_ENTRY_DESCRIPTOR_SIZE)
        .ok_or(EntryProductionValidationError::RecordRange)?;
    let actual = object
        .get(start..end)
        .ok_or(EntryProductionValidationError::RecordRange)?;
    let expected = expected_root_entry_record(plan);
    if let Some(offset) = actual.iter().zip(expected).position(|(a, e)| *a != e) {
        return Err(EntryProductionValidationError::RecordMismatch {
            offset: u16::try_from(offset).expect("root descriptor size fits u16"),
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryProductionValidationError {
    ObjectValidation(StrongSafepointRegistrationValidationError),
    LibraryRootDefinitions(Vec<ObjectDefinitionPlanId>),
    ExecutableRootDefinitions {
        expected: ObjectDefinitionPlanId,
        actual: Vec<ObjectDefinitionPlanId>,
    },
    MissingDefinitionAssignment(ObjectDefinitionPlanId),
    DefinitionAssignedToNonScoopMember(SlibMemberId),
    MissingVerifiedDefinition(ObjectDefinitionPlanId),
    RootAtomSet,
    AtomRange(super::SafepointRegistrationAtomFileRangeFailureV1),
    RootSection(BuiltinObjectSectionRoleV1),
    RootSize(u64),
    RootAlignment(u64),
    RecordRange,
    RecordMismatch {
        offset: u16,
        expected: u8,
        actual: u8,
    },
    MissingPatch(scoop_identity::DigestPatchIntentId),
    PatchMismatch(scoop_identity::DigestPatchIntentId),
    PatchSetMismatch,
    RelocationCount(usize),
    RelocationShape(u64),
    RelocationTarget(u64),
    MissingTargetSymbol(ObjectDefinitionPlanId),
    DefinitionIdentity(scoop_identity::ObjectDefinitionIdentityError),
    TargetOwner(super::StrongDefinitionOwnerValidationError),
    Hash(scoop_wire::HashError),
}

impl fmt::Display for EntryProductionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid executable entry object: {self:?}")
    }
}

impl std::error::Error for EntryProductionValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectValidation(source) => Some(source),
            Self::DefinitionIdentity(source) => Some(source),
            Self::TargetOwner(source) => Some(source),
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}
