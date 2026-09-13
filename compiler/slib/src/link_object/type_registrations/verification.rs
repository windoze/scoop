use std::collections::BTreeMap;

use super::digest::validate_digest_graph;
use super::physical::{atom_file_range, validate_objects, verified_member};
use super::record::{DESCRIPTOR_SIZE, validate_record_bytes};
use super::{
    StrongTypeRegistrationValidationError, TypeRegistrationPatchFailureV1,
    TypeRegistrationRelocationFailureV1,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    ScoopLirObjectCandidateV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedDarwinArm64RelocationFormV1, VerifiedMaterializedPatchSiteV1,
    VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestPatchIntentId, DigestSemanticFieldRole,
    PersistentExactTypeId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{StrongTypeRegistrationPlanSetV1, StrongTypeRegistrationPlanV1};

const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const DESCRIPTOR_POINTER_OFFSET: u64 = 168;
const DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET: u64 = 176;
const LAYOUT_FINGERPRINT_OFFSET: u64 = 208;
const DIGEST_WIDTH: u8 = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongTypeRegistrationV1 {
    exact_type: PersistentExactTypeId,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    descriptor_relocation: StrongRelocationBindingV1,
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
pub struct VerifiedStrongTypeRegistrationSetV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanSetV1,
    registrations: Vec<VerifiedStrongTypeRegistrationV1>,
}

impl VerifiedStrongTypeRegistrationSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &StrongTypeRegistrationPlanSetV1 {
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
    if patch_sites.producer() != plan.producer() {
        return Err(StrongTypeRegistrationValidationError::DigestPatchProducerMismatch);
    }

    let objects = validate_objects(patch_sites.builtins(), scoop_objects)?;
    let mut registrations = Vec::with_capacity(plan.registrations().len());
    for registration in plan.registrations() {
        validate_digest_graph(patch_sites.digest_plan(), *registration)?;
        registrations.push(verify_registration(&patch_sites, &objects, *registration)?);
    }

    Ok(VerifiedStrongTypeRegistrationSetV1 {
        patch_sites,
        plan,
        registrations,
    })
}

fn verify_registration(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: StrongTypeRegistrationPlanV1,
) -> Result<VerifiedStrongTypeRegistrationV1, StrongTypeRegistrationValidationError> {
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
        registration_definition_patch,
        descriptor_definition_patch,
        layout_fingerprint_patch,
    })
}

fn required_scoop_member(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongTypeRegistrationPlanV1,
    definition: scoop_identity::ObjectDefinitionPlanId,
) -> Result<SlibMemberId, StrongTypeRegistrationValidationError> {
    let member = builtins
        .member_plan()
        .member_for_definition(definition)
        .ok_or(
            StrongTypeRegistrationValidationError::MissingDefinitionAssignment {
                exact_type: plan.exact_type(),
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
            StrongTypeRegistrationValidationError::DefinitionAssignedToNonScoopMember {
                exact_type: plan.exact_type(),
                member,
            },
        );
    }
    Ok(member)
}

fn verify_layout_definition(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongTypeRegistrationPlanV1,
) -> Result<(), StrongTypeRegistrationValidationError> {
    let member = required_scoop_member(builtins, plan, plan.layout_definition_plan())?;
    let verified = verified_member(builtins, member)?;
    let definition = verified
        .definitions()
        .definition(plan.layout_definition_plan())
        .ok_or(
            StrongTypeRegistrationValidationError::MissingVerifiedDefinition {
                exact_type: plan.exact_type(),
                definition: plan.layout_definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.layout_primary_atom() {
        return Err(
            StrongTypeRegistrationValidationError::LayoutPrimaryAtomMismatch {
                exact_type: plan.exact_type(),
                expected: plan.layout_primary_atom(),
                actual: definition.primary_atom(),
            },
        );
    }
    verified
        .definitions()
        .strong_symbol_by_table_index(definition.primary_symbol_table_index())
        .ok_or(
            StrongTypeRegistrationValidationError::MissingLayoutPrimarySymbol {
                exact_type: plan.exact_type(),
            },
        )?;
    Ok(())
}

fn verify_descriptor_relocation(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    registration_member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: StrongTypeRegistrationPlanV1,
) -> Result<StrongRelocationBindingV1, StrongTypeRegistrationValidationError> {
    use TypeRegistrationRelocationFailureV1 as Failure;

    let physical = registration_member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == plan.primary_atom())
        .collect::<Vec<_>>();
    if physical.len() != 1 {
        return relocation_error(plan.exact_type(), Failure::Count);
    }
    let bindings = patch_sites
        .builtins()
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == registration_member.member()
                && binding.containing_atom() == plan.primary_atom()
        })
        .collect::<Vec<_>>();
    if bindings.len() != 1 {
        return relocation_error(plan.exact_type(), Failure::Count);
    }
    let binding = bindings[0];
    let kind = if binding.source_member() != registration_member.member() {
        Some(Failure::SourceMember)
    } else if binding.containing_atom() != plan.primary_atom() {
        Some(Failure::ContainingAtom)
    } else if binding.containing_atom_role() != DefinitionAtomRole::Primary {
        Some(Failure::ContainingAtomRole)
    } else if binding.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(Failure::SectionRole)
    } else if binding.offset_within_atom() != DESCRIPTOR_POINTER_OFFSET {
        Some(Failure::OffsetWithinAtom)
    } else if binding.width_bytes() != 8 {
        Some(Failure::Width)
    } else if binding.relocation_form() != VerifiedDarwinArm64RelocationFormV1::Unsigned64 {
        Some(Failure::Form)
    } else if binding.encoded_value() != 0 {
        Some(Failure::EncodedValue)
    } else if binding.target_slot() != RelocationTargetSlotV1::Single {
        Some(Failure::TargetSlot)
    } else {
        None
    };
    if let Some(kind) = kind {
        return relocation_error(plan.exact_type(), kind);
    }

    let descriptor_member = required_scoop_member(
        patch_sites.builtins(),
        plan,
        plan.descriptor_definition_plan(),
    )?;
    let descriptor_verified_member = verified_member(patch_sites.builtins(), descriptor_member)?;
    let descriptor_definition = descriptor_verified_member
        .definitions()
        .definition(plan.descriptor_definition_plan())
        .ok_or(
            StrongTypeRegistrationValidationError::MissingVerifiedDefinition {
                exact_type: plan.exact_type(),
                definition: plan.descriptor_definition_plan(),
            },
        )?;
    if descriptor_definition.primary_atom() != plan.descriptor_primary_atom() {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorPrimaryAtomMismatch {
                exact_type: plan.exact_type(),
                expected: plan.descriptor_primary_atom(),
                actual: descriptor_definition.primary_atom(),
            },
        );
    }
    let descriptor_symbol = descriptor_verified_member
        .definitions()
        .strong_symbol_by_table_index(descriptor_definition.primary_symbol_table_index())
        .ok_or(
            StrongTypeRegistrationValidationError::MissingDescriptorPrimarySymbol {
                exact_type: plan.exact_type(),
            },
        )?;
    let expected_owner = LinkDefinitionOwnerV1::from_strong_primary(
        StrongDefinitionEntity::exact_type(plan.exact_type()),
        StrongDefinitionRole::TypeDescriptor,
    )
    .expect("type descriptor is a valid strong definition owner");
    let (target_member, target_definition, target_owner) = match binding.resolution() {
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
            return relocation_error(plan.exact_type(), Failure::TargetDefinition);
        }
    };
    let kind = if target_definition != plan.descriptor_definition_plan() {
        Some(Failure::TargetDefinition)
    } else if target_member != descriptor_member {
        Some(Failure::TargetMember)
    } else if target_owner != expected_owner {
        Some(Failure::TargetOwner)
    } else if binding.symbol() != descriptor_symbol.macho_name() {
        Some(Failure::TargetSymbol)
    } else {
        None
    };
    if let Some(kind) = kind {
        return relocation_error(plan.exact_type(), kind);
    }
    Ok(binding.clone())
}

fn relocation_error<T>(
    exact_type: PersistentExactTypeId,
    kind: TypeRegistrationRelocationFailureV1,
) -> Result<T, StrongTypeRegistrationValidationError> {
    Err(StrongTypeRegistrationValidationError::DescriptorRelocationMismatch { exact_type, kind })
}

#[allow(clippy::too_many_arguments)]
fn require_patch(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanV1,
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

fn validate_exact_atom_patch_set(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanV1,
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
