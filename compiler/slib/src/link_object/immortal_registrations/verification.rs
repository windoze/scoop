use std::collections::BTreeMap;

use scoop_identity::{
    DefinitionAtomRole, DigestSemanticFieldRole, ObjectDefinitionPlanId, PersistentImmortalObjectId,
};
use scoop_lir::{
    StrongImmortalObjectRegistrationPlanSetV1, StrongImmortalObjectRegistrationPlanV1,
};

use super::digest::validate_digest_graph;
use super::physical::{atom_file_range, validate_objects, verified_member};
use super::record::{DESCRIPTOR_SIZE, validate_record_bytes};
use super::relocations::{
    registration_relocations, verify_object_relocation, verify_type_registration_relocation,
};
use super::{
    ImmortalObjectRegistrationPatchFailureV1, StrongImmortalObjectRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1, StrongRelocationBindingV1,
    VerifiedMaterializedPatchSiteV1, VerifiedScoopLirDigestPatchSiteSetV1,
};

const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const DIGEST_WIDTH: u8 = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectRegistrationV1 {
    object: PersistentImmortalObjectId,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    object_relocation: StrongRelocationBindingV1,
    type_registration_relocation: StrongRelocationBindingV1,
    registration_definition_patch: VerifiedMaterializedPatchSiteV1,
}

impl VerifiedStrongImmortalObjectRegistrationV1 {
    pub const fn object(&self) -> PersistentImmortalObjectId {
        self.object
    }

    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn primary_symbol_table_index(&self) -> u32 {
        self.primary_symbol_table_index
    }

    pub const fn checked_offset(&self) -> u64 {
        self.checked_offset
    }

    pub const fn object_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.object_relocation
    }

    pub const fn type_registration_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.type_registration_relocation
    }

    pub const fn registration_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.registration_definition_patch
    }
}

/// Proof that every final LIR immortal object has one exact provisional
/// registration whose two pointers resolve to its typed object and type
/// registration targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectRegistrationSetV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongImmortalObjectRegistrationPlanSetV1,
    registrations: Vec<VerifiedStrongImmortalObjectRegistrationV1>,
}

impl VerifiedStrongImmortalObjectRegistrationSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &StrongImmortalObjectRegistrationPlanSetV1 {
        &self.plan
    }

    pub fn registrations(&self) -> &[VerifiedStrongImmortalObjectRegistrationV1] {
        &self.registrations
    }
}

pub fn verify_strong_immortal_object_registrations_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongImmortalObjectRegistrationPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongImmortalObjectRegistrationSetV1,
    StrongImmortalObjectRegistrationValidationError,
> {
    if patch_sites.producer() != plan.producer() {
        return Err(StrongImmortalObjectRegistrationValidationError::DigestPatchProducerMismatch);
    }

    let objects = validate_objects(patch_sites.builtins(), scoop_objects)?;
    let mut registrations = Vec::with_capacity(plan.registrations().len());
    for registration in plan.registrations() {
        validate_digest_graph(patch_sites.digest_plan(), *registration)?;
        registrations.push(verify_registration(
            &patch_sites,
            &objects,
            plan.producer(),
            *registration,
        )?);
    }

    Ok(VerifiedStrongImmortalObjectRegistrationSetV1 {
        patch_sites,
        plan,
        registrations,
    })
}

fn verify_registration(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    producer: scoop_identity::ConeIdentity,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<
    VerifiedStrongImmortalObjectRegistrationV1,
    StrongImmortalObjectRegistrationValidationError,
> {
    let builtins = patch_sites.builtins();
    let member = required_scoop_member(builtins, plan, plan.registration_definition_plan())?;
    let verified_member = verified_member(builtins, member)?;
    let definition = verified_member
        .definitions()
        .definition(plan.registration_definition_plan())
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::MissingVerifiedDefinition {
                object: plan.object(),
                definition: plan.registration_definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.registration_primary_atom() {
        return Err(
            StrongImmortalObjectRegistrationValidationError::PrimaryAtomMismatch {
                object: plan.object(),
                expected: plan.registration_primary_atom(),
                actual: definition.primary_atom(),
            },
        );
    }
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.registration_primary_atom()
                && atom.atom_role() == DefinitionAtomRole::Primary
        })
        .copied()
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::MissingPrimaryAtom {
                object: plan.object(),
                atom: plan.registration_primary_atom(),
            },
        )?;
    let (section_role, file_start, file_end) =
        atom_file_range(verified_member, atom).map_err(|kind| {
            StrongImmortalObjectRegistrationValidationError::InvalidPrimaryAtomFileRange {
                object: plan.object(),
                atom: plan.registration_primary_atom(),
                kind,
            }
        })?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongImmortalObjectRegistrationValidationError::PrimaryAtomSectionMismatch(
                plan.object(),
            ),
        );
    }
    let size = file_end - file_start;
    if size != DESCRIPTOR_SIZE as u64 {
        return Err(
            StrongImmortalObjectRegistrationValidationError::PrimaryAtomSizeMismatch {
                object: plan.object(),
                actual: size,
            },
        );
    }

    let bindings = registration_relocations(patch_sites, verified_member, plan)?;
    let object_relocation = verify_object_relocation(patch_sites, plan, bindings[0])?;
    let type_registration_relocation =
        verify_type_registration_relocation(patch_sites, producer, plan, bindings[1])?;
    let registration_definition_patch = require_patch(
        patch_sites,
        plan,
        member,
        file_start,
        REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
    )?;
    validate_exact_atom_patch_set(patch_sites, plan, member)?;
    validate_record_bytes(objects[&member], file_start, plan)?;

    Ok(VerifiedStrongImmortalObjectRegistrationV1 {
        object: plan.object(),
        member,
        primary_symbol_table_index: definition.primary_symbol_table_index(),
        checked_offset: file_start,
        object_relocation,
        type_registration_relocation,
        registration_definition_patch,
    })
}

pub(super) fn required_scoop_member(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
    definition: ObjectDefinitionPlanId,
) -> Result<SlibMemberId, StrongImmortalObjectRegistrationValidationError> {
    let member = builtins
        .member_plan()
        .member_for_definition(definition)
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::MissingDefinitionAssignment {
                object: plan.object(),
                definition,
            },
        )?;
    if !builtins
        .member_plan()
        .scoop_lir_members()
        .iter()
        .any(|candidate| candidate.member_id() == member)
    {
        return Err(
            StrongImmortalObjectRegistrationValidationError::DefinitionAssignedToNonScoopMember {
                object: plan.object(),
                member,
            },
        );
    }
    Ok(member)
}

fn require_patch(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
    member: SlibMemberId,
    file_start: u64,
    offset_within_atom: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, StrongImmortalObjectRegistrationValidationError> {
    let intent = plan.registration_definition_patch();
    let patch = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == intent)
        .copied()
        .ok_or(
            StrongImmortalObjectRegistrationValidationError::MissingPatch {
                object: plan.object(),
                intent,
            },
        )?;
    let expected_checked_offset = file_start.checked_add(offset_within_atom).ok_or(
        StrongImmortalObjectRegistrationValidationError::RecordRangeOverflow(plan.object()),
    )?;
    let kind = if patch.source() != plan.registration_fingerprint_node() {
        Some(ImmortalObjectRegistrationPatchFailureV1::Source)
    } else if patch.semantic_field_role() != DigestSemanticFieldRole::RegistrationDefinition {
        Some(ImmortalObjectRegistrationPatchFailureV1::SemanticFieldRole)
    } else if patch.member() != member {
        Some(ImmortalObjectRegistrationPatchFailureV1::Member)
    } else if patch.definition() != plan.registration_definition_plan() {
        Some(ImmortalObjectRegistrationPatchFailureV1::Definition)
    } else if patch.atom() != plan.registration_primary_atom() {
        Some(ImmortalObjectRegistrationPatchFailureV1::Atom)
    } else if patch.atom_role() != DefinitionAtomRole::Primary {
        Some(ImmortalObjectRegistrationPatchFailureV1::AtomRole)
    } else if patch.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(ImmortalObjectRegistrationPatchFailureV1::SectionRole)
    } else if patch.offset_within_atom() != offset_within_atom {
        Some(ImmortalObjectRegistrationPatchFailureV1::OffsetWithinAtom)
    } else if patch.checked_offset() != expected_checked_offset {
        Some(ImmortalObjectRegistrationPatchFailureV1::CheckedOffset)
    } else if patch.width_bytes() != DIGEST_WIDTH {
        Some(ImmortalObjectRegistrationPatchFailureV1::Width)
    } else {
        None
    };
    if let Some(kind) = kind {
        return Err(
            StrongImmortalObjectRegistrationValidationError::PatchMismatch {
                object: plan.object(),
                intent,
                kind,
            },
        );
    }
    Ok(patch)
}

fn validate_exact_atom_patch_set(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongImmortalObjectRegistrationPlanV1,
    member: SlibMemberId,
) -> Result<(), StrongImmortalObjectRegistrationValidationError> {
    if let Some(site) = patch_sites.sites().iter().find(|site| {
        site.member() == member
            && site.atom() == plan.registration_primary_atom()
            && site.intent() != plan.registration_definition_patch()
    }) {
        return Err(
            StrongImmortalObjectRegistrationValidationError::UnexpectedPatchInPrimaryAtom {
                object: plan.object(),
                intent: site.intent(),
            },
        );
    }
    Ok(())
}
