mod descriptor;
mod registration;

use descriptor::verify_descriptor;
use registration::{required_scoop_member, verify_descriptor_relocation, verify_layout_definition};

use std::collections::BTreeMap;

use super::digest::validate_digest_graph;
use super::physical::{atom_file_range, validate_objects, verified_member};
use super::record::{DESCRIPTOR_SIZE, validate_record_bytes};
use super::{
    StrongTypeRegistrationValidationError, TypeDescriptorDiagnosticRelocationFailureV1,
    TypeDescriptorITableDirectoryFailureV1, TypeRegistrationPatchFailureV1,
    TypeRegistrationRelocationFailureV1,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    ScoopLirObjectCandidateV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedDarwinArm64RelocationFormV1, VerifiedDarwinArm64RelocationShapeV1,
    VerifiedMaterializedPatchSiteV1, VerifiedRelocationTargetV1, VerifiedRelocationUseV1,
    VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestPatchIntentId, DigestSemanticFieldRole, MangledSymbol,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolKey,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    StrongTypeDescriptorRefV1, StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV1,
    StrongTypeDispatchCallableRefV2, StrongTypeRegistrationPlan, StrongTypeRegistrationPlanSet,
    StrongTypeRegistrationPlanSetV1, TypeDescriptorITableDirectoryV1,
};

use super::versioned::{DescriptorReferenceKind, LinkDescriptorReference};

const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const DESCRIPTOR_POINTER_OFFSET: u64 = 168;
const DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET: u64 = 176;
const LAYOUT_FINGERPRINT_OFFSET: u64 = 208;
const DIGEST_WIDTH: u8 = 32;
const TYPE_DESCRIPTOR_SIZE: u64 = 144;
const TYPE_DESCRIPTOR_DIAGNOSTIC_POINTER_OFFSET: u64 = 112;
const TYPE_DESCRIPTOR_ITABLE_DIRECTORY_POINTER_OFFSET: u64 = 96;
const TYPE_DESCRIPTOR_ITABLE_ENTRY_SIZE: u64 = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeDescriptorV1 {
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    diagnostic_checked_offset: u64,
    diagnostic_size: u64,
    diagnostic_relocation: VerifiedRelocationUseV1,
    itable_directory: VerifiedTypeDescriptorITableDirectoryV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedTypeDescriptorITableDirectoryV1 {
    Null,
    Defined {
        checked_offset: u64,
        byte_size: u64,
        descriptor_relocation: Box<VerifiedRelocationUseV1>,
        entry_relocations: Vec<VerifiedRelocationUseV1>,
    },
}

impl VerifiedTypeDescriptorITableDirectoryV1 {
    pub const fn checked_offset(&self) -> Option<u64> {
        match self {
            Self::Null => None,
            Self::Defined { checked_offset, .. } => Some(*checked_offset),
        }
    }

    pub const fn byte_size(&self) -> u64 {
        match self {
            Self::Null => 0,
            Self::Defined { byte_size, .. } => *byte_size,
        }
    }

    pub fn descriptor_relocation(&self) -> Option<&VerifiedRelocationUseV1> {
        match self {
            Self::Null => None,
            Self::Defined {
                descriptor_relocation,
                ..
            } => Some(descriptor_relocation.as_ref()),
        }
    }

    pub fn entry_relocations(&self) -> &[VerifiedRelocationUseV1] {
        match self {
            Self::Null => &[],
            Self::Defined {
                entry_relocations, ..
            } => entry_relocations,
        }
    }
}

impl VerifiedStrongTypeDescriptorV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub const fn primary_symbol_table_index(&self) -> u32 {
        self.primary_symbol_table_index
    }

    pub const fn checked_offset(&self) -> u64 {
        self.checked_offset
    }

    pub const fn diagnostic_checked_offset(&self) -> u64 {
        self.diagnostic_checked_offset
    }

    pub const fn diagnostic_size(&self) -> u64 {
        self.diagnostic_size
    }

    pub const fn diagnostic_relocation(&self) -> &VerifiedRelocationUseV1 {
        &self.diagnostic_relocation
    }

    pub const fn itable_directory(&self) -> &VerifiedTypeDescriptorITableDirectoryV1 {
        &self.itable_directory
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeRegistrationV1 {
    exact_type: PersistentExactTypeId,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    descriptor_relocation: StrongRelocationBindingV1,
    descriptor: VerifiedStrongTypeDescriptorV1,
    registration_definition_patch: VerifiedMaterializedPatchSiteV1,
    descriptor_definition_patch: VerifiedMaterializedPatchSiteV1,
    layout_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
}

impl VerifiedStrongTypeRegistrationV1 {
    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
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

    pub const fn descriptor_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.descriptor_relocation
    }

    pub const fn descriptor(&self) -> &VerifiedStrongTypeDescriptorV1 {
        &self.descriptor
    }

    pub const fn registration_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.registration_definition_patch
    }

    pub const fn descriptor_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.descriptor_definition_patch
    }

    pub const fn layout_fingerprint_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.layout_fingerprint_patch
    }
}

/// Proof that every local exact type has one canonical provisional
/// registration record whose descriptor pointer resolves to its own typed
/// TypeDescriptor definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeRegistrationSetV1<
    D = StrongTypeDescriptorRefV1,
    C = StrongTypeDispatchCallableRefV1,
> {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanSet<D, C>,
    registrations: Vec<VerifiedStrongTypeRegistrationV1>,
}

pub type VerifiedStrongTypeRegistrationSetV2 =
    VerifiedStrongTypeRegistrationSetV1<StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV2>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone> VerifiedStrongTypeRegistrationSetV1<D, C> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &StrongTypeRegistrationPlanSet<D, C> {
        &self.plan
    }

    pub fn registrations(&self) -> &[VerifiedStrongTypeRegistrationV1] {
        &self.registrations
    }
}

pub fn verify_strong_type_registrations_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeRegistrationSetV1, StrongTypeRegistrationValidationError> {
    verify_strong_type_registrations(patch_sites, plan, scoop_objects)
}

pub fn verify_strong_type_registrations_v2(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: scoop_lir::StrongTypeRegistrationPlanSetV2,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeRegistrationSetV2, StrongTypeRegistrationValidationError> {
    verify_strong_type_registrations(patch_sites, plan, scoop_objects)
}

fn verify_strong_type_registrations<D, C>(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanSet<D, C>,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeRegistrationSetV1<D, C>, StrongTypeRegistrationValidationError>
where
    D: LinkDescriptorReference,
    C: Clone,
{
    if patch_sites.producer() != plan.producer() {
        return Err(StrongTypeRegistrationValidationError::DigestPatchProducerMismatch);
    }

    let objects = validate_objects(patch_sites.builtins(), scoop_objects)?;
    let plans_by_exact = plan
        .registrations()
        .iter()
        .map(|registration| (registration.exact_type(), registration))
        .collect::<BTreeMap<_, _>>();
    let mut registrations = Vec::with_capacity(plan.registrations().len());
    for registration in plan.registrations() {
        validate_digest_graph(patch_sites.digest_plan(), registration)?;
        registrations.push(verify_registration(
            &patch_sites,
            &objects,
            registration,
            &plans_by_exact,
        )?);
    }

    Ok(VerifiedStrongTypeRegistrationSetV1 {
        patch_sites,
        plan,
        registrations,
    })
}

fn verify_registration<D, C>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongTypeRegistrationPlan<D, C>,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlan<D, C>>,
) -> Result<VerifiedStrongTypeRegistrationV1, StrongTypeRegistrationValidationError>
where
    D: LinkDescriptorReference,
{
    let builtins = patch_sites.builtins();
    let member = required_scoop_member(builtins, plan, plan.definition_plan())?;
    let verified_member = verified_member(builtins, member)?;
    let definition = verified_member
        .definitions()
        .definition(plan.definition_plan())
        .ok_or(
            StrongTypeRegistrationValidationError::MissingVerifiedDefinition {
                exact_type: plan.exact_type(),
                definition: plan.definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.primary_atom() {
        return Err(StrongTypeRegistrationValidationError::PrimaryAtomMismatch {
            exact_type: plan.exact_type(),
            expected: plan.primary_atom(),
            actual: definition.primary_atom(),
        });
    }
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.primary_atom() && atom.atom_role() == DefinitionAtomRole::Primary
        })
        .copied()
        .ok_or(StrongTypeRegistrationValidationError::MissingPrimaryAtom {
            exact_type: plan.exact_type(),
            atom: plan.primary_atom(),
        })?;
    let (section_role, file_start, file_end) =
        atom_file_range(verified_member, atom).map_err(|kind| {
            StrongTypeRegistrationValidationError::InvalidPrimaryAtomFileRange {
                exact_type: plan.exact_type(),
                atom: plan.primary_atom(),
                kind,
            }
        })?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongTypeRegistrationValidationError::PrimaryAtomSectionMismatch {
                exact_type: plan.exact_type(),
            },
        );
    }
    let size = file_end - file_start;
    if size != DESCRIPTOR_SIZE as u64 {
        return Err(
            StrongTypeRegistrationValidationError::PrimaryAtomSizeMismatch {
                exact_type: plan.exact_type(),
                actual: size,
            },
        );
    }

    verify_layout_definition(builtins, plan)?;
    let descriptor = verify_descriptor(patch_sites, objects, plan, plans_by_exact)?;
    let descriptor_relocation = verify_descriptor_relocation(patch_sites, verified_member, plan)?;
    let registration_definition_patch = require_patch(
        patch_sites,
        plan,
        plan.registration_definition_patch(),
        plan.registration_fingerprint_node(),
        DigestSemanticFieldRole::RegistrationDefinition,
        member,
        file_start,
        REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
    )?;
    let descriptor_definition_patch = require_patch(
        patch_sites,
        plan,
        plan.descriptor_definition_patch(),
        plan.descriptor_definition_node(),
        DigestSemanticFieldRole::DescriptorDefinition,
        member,
        file_start,
        DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET,
    )?;
    let layout_fingerprint_patch = require_patch(
        patch_sites,
        plan,
        plan.layout_fingerprint_patch(),
        plan.layout_fingerprint_node(),
        DigestSemanticFieldRole::Layout,
        member,
        file_start,
        LAYOUT_FINGERPRINT_OFFSET,
    )?;
    validate_exact_atom_patch_set(patch_sites, plan, member)?;
    validate_record_bytes(objects[&member], file_start, plan)?;

    Ok(VerifiedStrongTypeRegistrationV1 {
        exact_type: plan.exact_type(),
        member,
        primary_symbol_table_index: definition.primary_symbol_table_index(),
        checked_offset: file_start,
        descriptor_relocation,
        descriptor,
        registration_definition_patch,
        descriptor_definition_patch,
        layout_fingerprint_patch,
    })
}

fn relocation_error<T>(
    exact_type: PersistentExactTypeId,
    kind: TypeRegistrationRelocationFailureV1,
) -> Result<T, StrongTypeRegistrationValidationError> {
    Err(StrongTypeRegistrationValidationError::DescriptorRelocationMismatch { exact_type, kind })
}

#[allow(clippy::too_many_arguments)]
fn require_patch<D: Copy, C>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
    intent: DigestPatchIntentId,
    source: DigestNodeId,
    field_role: DigestSemanticFieldRole,
    member: SlibMemberId,
    file_start: u64,
    offset_within_atom: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, StrongTypeRegistrationValidationError> {
    let patch = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == intent)
        .copied()
        .ok_or(StrongTypeRegistrationValidationError::MissingPatch {
            exact_type: plan.exact_type(),
            intent,
        })?;
    let expected_checked_offset = file_start.checked_add(offset_within_atom).ok_or(
        StrongTypeRegistrationValidationError::RecordRangeOverflow(plan.exact_type()),
    )?;
    let kind = if patch.source() != source {
        Some(TypeRegistrationPatchFailureV1::Source)
    } else if patch.semantic_field_role() != field_role {
        Some(TypeRegistrationPatchFailureV1::SemanticFieldRole)
    } else if patch.member() != member {
        Some(TypeRegistrationPatchFailureV1::Member)
    } else if patch.definition() != plan.definition_plan() {
        Some(TypeRegistrationPatchFailureV1::Definition)
    } else if patch.atom() != plan.primary_atom() {
        Some(TypeRegistrationPatchFailureV1::Atom)
    } else if patch.atom_role() != DefinitionAtomRole::Primary {
        Some(TypeRegistrationPatchFailureV1::AtomRole)
    } else if patch.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(TypeRegistrationPatchFailureV1::SectionRole)
    } else if patch.offset_within_atom() != offset_within_atom {
        Some(TypeRegistrationPatchFailureV1::OffsetWithinAtom)
    } else if patch.checked_offset() != expected_checked_offset {
        Some(TypeRegistrationPatchFailureV1::CheckedOffset)
    } else if patch.width_bytes() != DIGEST_WIDTH {
        Some(TypeRegistrationPatchFailureV1::Width)
    } else {
        None
    };
    if let Some(kind) = kind {
        return Err(StrongTypeRegistrationValidationError::PatchMismatch {
            exact_type: plan.exact_type(),
            intent,
            kind,
        });
    }
    Ok(patch)
}

fn validate_exact_atom_patch_set<D: Copy, C>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
    member: SlibMemberId,
) -> Result<(), StrongTypeRegistrationValidationError> {
    let expected = [
        plan.registration_definition_patch(),
        plan.descriptor_definition_patch(),
        plan.layout_fingerprint_patch(),
    ];
    if let Some(site) = patch_sites.sites().iter().find(|site| {
        site.member() == member
            && site.atom() == plan.primary_atom()
            && !expected.contains(&site.intent())
    }) {
        return Err(
            StrongTypeRegistrationValidationError::UnexpectedPatchInPrimaryAtom {
                exact_type: plan.exact_type(),
                intent: site.intent(),
            },
        );
    }
    Ok(())
}
