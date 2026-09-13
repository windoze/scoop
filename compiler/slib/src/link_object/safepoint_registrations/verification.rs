use std::collections::BTreeMap;

use super::physical::{atom_file_range, validate_objects, verified_member};
use super::record::{DESCRIPTOR_SIZE, validate_record_bytes};
use super::{
    SafepointRegistrationDigestPlanFailureV1, SafepointRegistrationPatchFailureV1,
    SafepointRegistrationSemanticFieldV1, StrongSafepointRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1, VerifiedMaterializedPatchSiteV1,
    VerifiedScoopLirDigestPatchSiteSetV1, VerifiedScoopLirStackmapRecordV1,
    VerifiedScoopLirStackmapSetV1,
};
use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId, DigestSemanticFieldRole,
    PersistentSafepointSiteId,
};
use scoop_lir::{
    DigestInputRefV1, StrongDigestFinalizationPlanV1, StrongSafepointRegistrationPlanSetV1,
    StrongSafepointRegistrationPlanV1,
};

const DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const NORMALIZED_STACKMAP_FINGERPRINT_OFFSET: u64 = 200;
const DIGEST_WIDTH: u8 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointRegistrationV1 {
    site: PersistentSafepointSiteId,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    registration_definition_patch: VerifiedMaterializedPatchSiteV1,
    normalized_stackmap_patch: VerifiedMaterializedPatchSiteV1,
}

impl VerifiedStrongSafepointRegistrationV1 {
    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn member(self) -> SlibMemberId {
        self.member
    }

    pub const fn primary_symbol_table_index(self) -> u32 {
        self.primary_symbol_table_index
    }

    pub const fn checked_offset(self) -> u64 {
        self.checked_offset
    }

    pub const fn registration_definition_patch(self) -> VerifiedMaterializedPatchSiteV1 {
        self.registration_definition_patch
    }

    pub const fn normalized_stackmap_patch(self) -> VerifiedMaterializedPatchSiteV1 {
        self.normalized_stackmap_patch
    }
}

/// Proof that every LIR safepoint has exactly one canonical provisional
/// registration record in the verified Scoop object set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongSafepointRegistrationSetV1 {
    stackmaps: VerifiedScoopLirStackmapSetV1,
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongSafepointRegistrationPlanSetV1,
    registrations: Vec<VerifiedStrongSafepointRegistrationV1>,
}

impl VerifiedStrongSafepointRegistrationSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn stackmaps(&self) -> &VerifiedScoopLirStackmapSetV1 {
        &self.stackmaps
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &StrongSafepointRegistrationPlanSetV1 {
        &self.plan
    }

    pub fn registrations(&self) -> &[VerifiedStrongSafepointRegistrationV1] {
        &self.registrations
    }
}

pub fn verify_strong_safepoint_registrations_v1(
    stackmaps: VerifiedScoopLirStackmapSetV1,
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongSafepointRegistrationPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongSafepointRegistrationSetV1, StrongSafepointRegistrationValidationError> {
    if stackmaps.producer() != plan.producer() {
        return Err(StrongSafepointRegistrationValidationError::StackmapProducerMismatch);
    }
    if patch_sites.producer() != plan.producer() {
        return Err(StrongSafepointRegistrationValidationError::DigestPatchProducerMismatch);
    }
    if stackmaps.builtins() != patch_sites.builtins() {
        return Err(StrongSafepointRegistrationValidationError::ObjectProofMismatch);
    }

    let objects = validate_objects(stackmaps.builtins(), scoop_objects)?;
    validate_site_coverage(&plan, &stackmaps)?;
    let mut registrations = Vec::with_capacity(plan.registrations().len());
    for (planned, stackmap) in plan.registrations().iter().zip(stackmaps.records()) {
        validate_stackmap_semantics(*planned, stackmap)?;
        validate_digest_graph(patch_sites.digest_plan(), *planned)?;
        registrations.push(verify_registration(
            stackmaps.builtins(),
            &patch_sites,
            &objects,
            *planned,
        )?);
    }

    Ok(VerifiedStrongSafepointRegistrationSetV1 {
        stackmaps,
        patch_sites,
        plan,
        registrations,
    })
}

fn validate_digest_graph(
    digest_plan: &StrongDigestFinalizationPlanV1,
    plan: StrongSafepointRegistrationPlanV1,
) -> Result<(), StrongSafepointRegistrationValidationError> {
    use SafepointRegistrationDigestPlanFailureV1 as Failure;

    let object_key = DigestNodeKey::object_definition(plan.primary_atom());
    let object = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &object_key)
        .ok_or(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::MissingObjectDefinitionNode,
            },
        )?;
    if !object.direct_inputs().is_empty() {
        return Err(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::ObjectDefinitionDirectInputs,
            },
        );
    }
    if !object.patch_intents().is_empty() {
        return Err(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::ObjectDefinitionPatchSet,
            },
        );
    }
    let stackmap_key = DigestNodeKey::stackmap_record(plan.site());
    let stackmap = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &stackmap_key)
        .ok_or(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::MissingStackmapNode,
            },
        )?;
    if stackmap.id() != plan.normalized_stackmap_fingerprint_node() {
        return Err(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::StackmapNodeIdentity,
            },
        );
    }
    let registration_key = DigestNodeKey::strong_registration(plan.definition_plan());
    let registration = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &registration_key)
        .ok_or(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::MissingRegistrationNode,
            },
        )?;
    if registration.id() != plan.registration_fingerprint_node() {
        return Err(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::RegistrationNodeIdentity,
            },
        );
    }
    if registration.direct_inputs()
        != [
            DigestInputRefV1::from_node(object),
            DigestInputRefV1::from_node(stackmap),
        ]
    {
        return Err(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::RegistrationDirectInputs,
            },
        );
    }
    if registration.patch_intents().len() != 1
        || registration.patch_intents()[0].id() != plan.registration_definition_patch()
    {
        return Err(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::RegistrationPatchSet,
            },
        );
    }
    if stackmap.patch_intents().len() != 1
        || stackmap.patch_intents()[0].id() != plan.normalized_stackmap_patch()
    {
        return Err(
            StrongSafepointRegistrationValidationError::DigestPlanMismatch {
                site: plan.site(),
                kind: Failure::StackmapPatchSet,
            },
        );
    }
    Ok(())
}

fn validate_site_coverage(
    plan: &StrongSafepointRegistrationPlanSetV1,
    stackmaps: &VerifiedScoopLirStackmapSetV1,
) -> Result<(), StrongSafepointRegistrationValidationError> {
    let expected = plan
        .registrations()
        .iter()
        .map(|registration| registration.site())
        .collect::<Vec<_>>();
    let actual = stackmaps
        .records()
        .iter()
        .map(|record| record.normalized().canonical().site())
        .collect::<Vec<_>>();
    if expected != actual {
        return Err(
            StrongSafepointRegistrationValidationError::SiteCoverageMismatch { expected, actual },
        );
    }
    Ok(())
}

fn validate_stackmap_semantics(
    plan: StrongSafepointRegistrationPlanV1,
    stackmap: &VerifiedScoopLirStackmapRecordV1,
) -> Result<(), StrongSafepointRegistrationValidationError> {
    let actual = stackmap.normalized().canonical();
    let mismatch = if actual.safepoint_id() != plan.safepoint().get() {
        Some(SafepointRegistrationSemanticFieldV1::SafepointId)
    } else if actual.owner() != plan.owner() {
        Some(SafepointRegistrationSemanticFieldV1::OwnerCallable)
    } else if actual.role() != plan.role() {
        Some(SafepointRegistrationSemanticFieldV1::SiteRole)
    } else if actual.root_pair_count() != plan.root_pair_count() {
        Some(SafepointRegistrationSemanticFieldV1::RootPairCount)
    } else {
        None
    };
    if let Some(field) = mismatch {
        return Err(
            StrongSafepointRegistrationValidationError::StackmapSemanticMismatch {
                site: plan.site(),
                field,
            },
        );
    }
    Ok(())
}

fn verify_registration(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: StrongSafepointRegistrationPlanV1,
) -> Result<VerifiedStrongSafepointRegistrationV1, StrongSafepointRegistrationValidationError> {
    let member = builtins
        .member_plan()
        .member_for_definition(plan.definition_plan())
        .ok_or(
            StrongSafepointRegistrationValidationError::MissingDefinitionAssignment {
                site: plan.site(),
                definition: plan.definition_plan(),
            },
        )?;
    if !builtins
        .member_plan()
        .scoop_lir_members()
        .iter()
        .any(|candidate| candidate.member_id() == member)
    {
        return Err(
            StrongSafepointRegistrationValidationError::DefinitionAssignedToNonScoopMember {
                site: plan.site(),
                member,
            },
        );
    }
    let verified_member = verified_member(builtins, member)?;
    let definition = verified_member
        .definitions()
        .definition(plan.definition_plan())
        .ok_or(
            StrongSafepointRegistrationValidationError::MissingVerifiedDefinition {
                site: plan.site(),
                definition: plan.definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.primary_atom() {
        return Err(
            StrongSafepointRegistrationValidationError::PrimaryAtomMismatch {
                site: plan.site(),
                expected: plan.primary_atom(),
                actual: definition.primary_atom(),
            },
        );
    }
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.primary_atom() && atom.atom_role() == DefinitionAtomRole::Primary
        })
        .copied()
        .ok_or(
            StrongSafepointRegistrationValidationError::MissingPrimaryAtom {
                site: plan.site(),
                atom: plan.primary_atom(),
            },
        )?;
    let (section_role, file_start, file_end) =
        atom_file_range(verified_member, atom).map_err(|kind| {
            StrongSafepointRegistrationValidationError::InvalidPrimaryAtomFileRange {
                site: plan.site(),
                atom: plan.primary_atom(),
                kind,
            }
        })?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongSafepointRegistrationValidationError::PrimaryAtomSectionMismatch {
                site: plan.site(),
            },
        );
    }
    let size = file_end - file_start;
    if size != DESCRIPTOR_SIZE as u64 {
        return Err(
            StrongSafepointRegistrationValidationError::PrimaryAtomSizeMismatch {
                site: plan.site(),
                actual: size,
            },
        );
    }
    if let Some(relocation) = verified_member
        .relocations()
        .iter()
        .find(|relocation| relocation.containing_atom() == plan.primary_atom())
    {
        return Err(
            StrongSafepointRegistrationValidationError::UnexpectedRelocation {
                site: plan.site(),
                offset_within_atom: relocation.offset_within_atom(),
            },
        );
    }

    let registration_definition_patch = require_patch(
        patch_sites,
        plan,
        plan.registration_definition_patch(),
        plan.registration_fingerprint_node(),
        DigestSemanticFieldRole::RegistrationDefinition,
        member,
        file_start,
        DEFINITION_FINGERPRINT_OFFSET,
    )?;
    let normalized_stackmap_patch = require_patch(
        patch_sites,
        plan,
        plan.normalized_stackmap_patch(),
        plan.normalized_stackmap_fingerprint_node(),
        DigestSemanticFieldRole::NormalizedStackmap,
        member,
        file_start,
        NORMALIZED_STACKMAP_FINGERPRINT_OFFSET,
    )?;
    validate_exact_atom_patch_set(patch_sites, plan, member)?;
    validate_record_bytes(objects[&member], file_start, plan)?;

    Ok(VerifiedStrongSafepointRegistrationV1 {
        site: plan.site(),
        member,
        primary_symbol_table_index: definition.primary_symbol_table_index(),
        checked_offset: file_start,
        registration_definition_patch,
        normalized_stackmap_patch,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_patch(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongSafepointRegistrationPlanV1,
    intent: DigestPatchIntentId,
    source: DigestNodeId,
    field_role: DigestSemanticFieldRole,
    member: SlibMemberId,
    file_start: u64,
    offset_within_atom: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, StrongSafepointRegistrationValidationError> {
    let patch = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == intent)
        .copied()
        .ok_or(StrongSafepointRegistrationValidationError::MissingPatch {
            site: plan.site(),
            intent,
        })?;
    let expected_checked_offset = file_start
        .checked_add(offset_within_atom)
        .ok_or(StrongSafepointRegistrationValidationError::RecordRangeOverflow(plan.site()))?;
    let kind = if patch.source() != source {
        Some(SafepointRegistrationPatchFailureV1::Source)
    } else if patch.semantic_field_role() != field_role {
        Some(SafepointRegistrationPatchFailureV1::SemanticFieldRole)
    } else if patch.member() != member {
        Some(SafepointRegistrationPatchFailureV1::Member)
    } else if patch.definition() != plan.definition_plan() {
        Some(SafepointRegistrationPatchFailureV1::Definition)
    } else if patch.atom() != plan.primary_atom() {
        Some(SafepointRegistrationPatchFailureV1::Atom)
    } else if patch.atom_role() != DefinitionAtomRole::Primary {
        Some(SafepointRegistrationPatchFailureV1::AtomRole)
    } else if patch.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(SafepointRegistrationPatchFailureV1::SectionRole)
    } else if patch.offset_within_atom() != offset_within_atom {
        Some(SafepointRegistrationPatchFailureV1::OffsetWithinAtom)
    } else if patch.checked_offset() != expected_checked_offset {
        Some(SafepointRegistrationPatchFailureV1::CheckedOffset)
    } else if patch.width_bytes() != DIGEST_WIDTH {
        Some(SafepointRegistrationPatchFailureV1::Width)
    } else {
        None
    };
    if let Some(kind) = kind {
        return Err(StrongSafepointRegistrationValidationError::PatchMismatch {
            site: plan.site(),
            intent,
            kind,
        });
    }
    Ok(patch)
}

fn validate_exact_atom_patch_set(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongSafepointRegistrationPlanV1,
    member: SlibMemberId,
) -> Result<(), StrongSafepointRegistrationValidationError> {
    let expected = [
        plan.registration_definition_patch(),
        plan.normalized_stackmap_patch(),
    ];
    if let Some(site) = patch_sites.sites().iter().find(|site| {
        site.member() == member
            && site.atom() == plan.primary_atom()
            && !expected.contains(&site.intent())
    }) {
        return Err(
            StrongSafepointRegistrationValidationError::UnexpectedPatchInPrimaryAtom {
                site: plan.site(),
                intent: site.intent(),
            },
        );
    }
    Ok(())
}
