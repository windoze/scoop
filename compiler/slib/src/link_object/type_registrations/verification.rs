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
    StrongTypeDescriptorRefV1, StrongTypeRegistrationPlanSetV1, StrongTypeRegistrationPlanV1,
    TypeDescriptorITableDirectoryV1,
};

const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const DESCRIPTOR_POINTER_OFFSET: u64 = 168;
const DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET: u64 = 176;
const LAYOUT_FINGERPRINT_OFFSET: u64 = 208;
const DIGEST_WIDTH: u8 = 32;
const TYPE_DESCRIPTOR_SIZE: u64 = 128;
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

fn verify_registration(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongTypeRegistrationPlanV1,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlanV1>,
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

fn verify_descriptor(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongTypeRegistrationPlanV1,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlanV1>,
) -> Result<VerifiedStrongTypeDescriptorV1, StrongTypeRegistrationValidationError> {
    let member = required_scoop_member(
        patch_sites.builtins(),
        plan,
        plan.descriptor_definition_plan(),
    )?;
    let verified = verified_member(patch_sites.builtins(), member)?;
    let definition = verified
        .definitions()
        .definition(plan.descriptor_definition_plan())
        .ok_or(
            StrongTypeRegistrationValidationError::MissingVerifiedDefinition {
                exact_type: plan.exact_type(),
                definition: plan.descriptor_definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.descriptor_primary_atom() {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorPrimaryAtomMismatch {
                exact_type: plan.exact_type(),
                expected: plan.descriptor_primary_atom(),
                actual: definition.primary_atom(),
            },
        );
    }
    let primary = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.descriptor_primary_atom()
                && atom.atom_role() == DefinitionAtomRole::Primary
        })
        .copied()
        .ok_or(
            StrongTypeRegistrationValidationError::MissingDescriptorPrimaryAtom {
                exact_type: plan.exact_type(),
                atom: plan.descriptor_primary_atom(),
            },
        )?;
    let (primary_section, primary_start, primary_end) = atom_file_range(verified, primary)
        .map_err(|kind| {
            StrongTypeRegistrationValidationError::InvalidDescriptorPrimaryAtomFileRange {
                exact_type: plan.exact_type(),
                atom: plan.descriptor_primary_atom(),
                kind,
            }
        })?;
    if primary_section != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorPrimaryAtomSectionMismatch {
                exact_type: plan.exact_type(),
            },
        );
    }
    let primary_size = primary_end - primary_start;
    if primary_size != TYPE_DESCRIPTOR_SIZE {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorPrimaryAtomSizeMismatch {
                exact_type: plan.exact_type(),
                actual: primary_size,
            },
        );
    }

    let diagnostic = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.diagnostic_atom()
                && atom.atom_role() == DefinitionAtomRole::AddressTakenConstant
        })
        .copied()
        .ok_or(
            StrongTypeRegistrationValidationError::MissingDescriptorDiagnosticAtom {
                exact_type: plan.exact_type(),
                atom: plan.diagnostic_atom(),
            },
        )?;
    let (diagnostic_section, diagnostic_start, diagnostic_end) =
        atom_file_range(verified, diagnostic).map_err(|kind| {
            StrongTypeRegistrationValidationError::InvalidDescriptorDiagnosticAtomFileRange {
                exact_type: plan.exact_type(),
                atom: plan.diagnostic_atom(),
                kind,
            }
        })?;
    if diagnostic_section != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorDiagnosticAtomSectionMismatch {
                exact_type: plan.exact_type(),
            },
        );
    }
    let expected_diagnostic = plan.semantic().diagnostic_name().as_bytes();
    let diagnostic_size = diagnostic_end - diagnostic_start;
    let expected_size =
        u64::try_from(expected_diagnostic.len()).expect("diagnostic length fits u64");
    if diagnostic_size != expected_size {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorDiagnosticAtomSizeMismatch {
                exact_type: plan.exact_type(),
                expected: expected_size,
                actual: diagnostic_size,
            },
        );
    }
    let diagnostic_start_index =
        usize::try_from(diagnostic_start).expect("object offset fits usize");
    let diagnostic_end_index = usize::try_from(diagnostic_end).expect("object offset fits usize");
    let actual_diagnostic = &objects[&member][diagnostic_start_index..diagnostic_end_index];
    if let Some(offset) = actual_diagnostic
        .iter()
        .zip(expected_diagnostic)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(
            StrongTypeRegistrationValidationError::DescriptorDiagnosticByteMismatch {
                exact_type: plan.exact_type(),
                offset_within_atom: u64::try_from(offset).expect("diagnostic offset fits u64"),
                expected: expected_diagnostic[offset],
                actual: actual_diagnostic[offset],
            },
        );
    }

    let diagnostic_relocation = verify_descriptor_diagnostic_relocation(
        verified,
        plan,
        diagnostic.section_ordinal(),
        diagnostic.start(),
    )?;
    let itable_directory = verify_itable_directory(
        patch_sites.builtins(),
        verified,
        definition,
        objects[&member],
        plan,
        plans_by_exact,
    )?;
    Ok(VerifiedStrongTypeDescriptorV1 {
        member,
        primary_symbol_table_index: definition.primary_symbol_table_index(),
        checked_offset: primary_start,
        diagnostic_checked_offset: diagnostic_start,
        diagnostic_size,
        diagnostic_relocation,
        itable_directory,
    })
}

fn verify_itable_directory(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    definition: &crate::link_object::VerifiedStrongObjectDefinitionV1,
    object: &[u8],
    plan: &StrongTypeRegistrationPlanV1,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlanV1>,
) -> Result<VerifiedTypeDescriptorITableDirectoryV1, StrongTypeRegistrationValidationError> {
    use TypeDescriptorITableDirectoryFailureV1 as Failure;

    let TypeDescriptorITableDirectoryV1::Defined(directory_atom) = plan.itable_directory() else {
        return if plan.semantic().itables().is_empty() {
            Ok(VerifiedTypeDescriptorITableDirectoryV1::Null)
        } else {
            itable_directory_error(plan, None, Failure::UnexpectedPlanBranch)
        };
    };
    if plan.semantic().itables().is_empty() {
        return itable_directory_error(plan, None, Failure::UnexpectedPlanBranch);
    }
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == directory_atom && atom.atom_role() == DefinitionAtomRole::RuntimeRecord
        })
        .copied()
        .ok_or_else(|| itable_directory_failure(plan, None, Failure::MissingAtom))?;
    let (section, start, end) = atom_file_range(member, atom)
        .map_err(|_| itable_directory_failure(plan, None, Failure::AtomFileRange))?;
    if section != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return itable_directory_error(plan, None, Failure::SectionRole);
    }
    let expected_size = u64::try_from(plan.semantic().itables().len())
        .ok()
        .and_then(|count| count.checked_mul(TYPE_DESCRIPTOR_ITABLE_ENTRY_SIZE))
        .ok_or_else(|| itable_directory_failure(plan, None, Failure::Size))?;
    if end.checked_sub(start) != Some(expected_size) {
        return itable_directory_error(plan, None, Failure::Size);
    }
    let start_index = usize::try_from(start)
        .map_err(|_| itable_directory_failure(plan, None, Failure::AtomFileRange))?;
    let end_index = usize::try_from(end)
        .map_err(|_| itable_directory_failure(plan, None, Failure::AtomFileRange))?;
    if object
        .get(start_index..end_index)
        .ok_or_else(|| itable_directory_failure(plan, None, Failure::AtomFileRange))?
        .iter()
        .any(|byte| *byte != 0)
    {
        return itable_directory_error(plan, None, Failure::NonzeroByte);
    }

    let descriptor_relocation = verify_itable_directory_pointer(
        member,
        plan,
        directory_atom,
        atom.section_ordinal(),
        atom.start(),
    )?;
    let mut relocations = member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == directory_atom)
        .cloned()
        .collect::<Vec<_>>();
    relocations.sort_unstable_by_key(VerifiedRelocationUseV1::offset_within_atom);
    let expected_offsets = plan
        .semantic()
        .itables()
        .iter()
        .enumerate()
        .flat_map(|(index, itable)| {
            let base = u64::try_from(index).expect("itable index fits u64")
                * TYPE_DESCRIPTOR_ITABLE_ENTRY_SIZE;
            std::iter::once(base).chain((!itable.slots().is_empty()).then_some(base + 8))
        })
        .collect::<Vec<_>>();
    if relocations
        .iter()
        .map(VerifiedRelocationUseV1::offset_within_atom)
        .ne(expected_offsets.iter().copied())
    {
        return itable_directory_error(plan, None, Failure::RelocationSet);
    }
    let mut relocation_index = 0;
    for (entry_index, itable) in plan.semantic().itables().iter().enumerate() {
        let interface = &relocations[relocation_index];
        relocation_index += 1;
        validate_itable_relocation_shape(plan, entry_index, interface)?;
        if !interface_target_matches(interface.shape(), itable.interface(), plans_by_exact) {
            return itable_directory_error(plan, Some(entry_index), Failure::InterfaceTarget);
        }
        if !itable.slots().is_empty() {
            let slots = &relocations[relocation_index];
            relocation_index += 1;
            validate_itable_relocation_shape(plan, entry_index, slots)?;
            let expected =
                dispatch_definition(builtins.producer(), itable.table()).map_err(|_| {
                    itable_directory_failure(
                        plan,
                        Some(entry_index),
                        Failure::DispatchDefinitionIdentity,
                    )
                })?;
            if !matches!(
                slots.shape(),
                VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                    target: VerifiedRelocationTargetV1::StrongDefinition { definition }
                } if *definition == expected
            ) {
                return itable_directory_error(plan, Some(entry_index), Failure::SlotsTarget);
            }
        }
    }
    Ok(VerifiedTypeDescriptorITableDirectoryV1::Defined {
        checked_offset: start,
        byte_size: expected_size,
        descriptor_relocation: Box::new(descriptor_relocation),
        entry_relocations: relocations,
    })
}

fn verify_itable_directory_pointer(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongTypeRegistrationPlanV1,
    directory_atom: scoop_identity::ObjectDefinitionAtomId,
    directory_section: std::num::NonZeroU8,
    directory_value: u64,
) -> Result<VerifiedRelocationUseV1, StrongTypeRegistrationValidationError> {
    use TypeDescriptorITableDirectoryFailureV1 as Failure;
    let relocations = member
        .relocations()
        .iter()
        .filter(|relocation| {
            relocation.containing_atom() == plan.descriptor_primary_atom()
                && relocation.offset_within_atom()
                    == TYPE_DESCRIPTOR_ITABLE_DIRECTORY_POINTER_OFFSET
        })
        .collect::<Vec<_>>();
    let [relocation] = relocations.as_slice() else {
        return itable_directory_error(plan, None, Failure::DescriptorRelocation);
    };
    let matches = relocation.containing_atom_role() == DefinitionAtomRole::Primary
        && relocation.section_role() == BuiltinObjectSectionRoleV1::ReadOnlyData
        && relocation.width_bytes() == 8
        && relocation.encoded_value() == 0
        && matches!(
            relocation.shape(),
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                target: VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: Some(owner_atom),
                    section_ordinal,
                    value,
                    ..
                }
            } if *owner_atom == directory_atom
                && *section_ordinal == directory_section
                && *value == directory_value
        );
    if !matches {
        return itable_directory_error(plan, None, Failure::DescriptorRelocation);
    }
    Ok((*relocation).clone())
}

fn validate_itable_relocation_shape(
    plan: &StrongTypeRegistrationPlanV1,
    entry: usize,
    relocation: &VerifiedRelocationUseV1,
) -> Result<(), StrongTypeRegistrationValidationError> {
    use TypeDescriptorITableDirectoryFailureV1 as Failure;
    if relocation.containing_atom_role() != DefinitionAtomRole::RuntimeRecord
        || relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
        || relocation.width_bytes() != 8
        || relocation.encoded_value() != 0
        || !matches!(
            relocation.shape(),
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { .. }
        )
    {
        return itable_directory_error(plan, Some(entry), Failure::RelocationShape);
    }
    Ok(())
}

fn interface_target_matches(
    shape: &VerifiedDarwinArm64RelocationShapeV1,
    interface: StrongTypeDescriptorRefV1,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlanV1>,
) -> bool {
    match interface {
        StrongTypeDescriptorRefV1::Local(exact_type) => {
            let Some(expected) = plans_by_exact.get(&exact_type) else {
                return false;
            };
            matches!(
                shape,
                VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                    target: VerifiedRelocationTargetV1::StrongDefinition { definition }
                } if *definition == expected.descriptor_definition_plan()
            )
        }
        StrongTypeDescriptorRefV1::CoreExternal(exact_type) => {
            let expected = expected_type_descriptor_macho_name(exact_type);
            matches!(
                shape,
                VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                    target: VerifiedRelocationTargetV1::ExternalUndefined { name, .. }
                } if name == &expected
            )
        }
    }
}

fn dispatch_definition(
    producer: scoop_identity::ConeIdentity,
    table: scoop_identity::PersistentDispatchTableId,
) -> Result<ObjectDefinitionPlanId, ()> {
    let key = ObjectDefinitionPlanKey::strong(
        producer,
        StrongDefinitionEntity::dispatch_table(table),
        StrongDefinitionRole::DispatchTable,
    )
    .map_err(|_| ())?;
    ObjectDefinitionPlanId::from_key(&key).map_err(|_| ())
}

fn expected_type_descriptor_macho_name(exact_type: PersistentExactTypeId) -> Vec<u8> {
    let symbol = MangledSymbol::from_key(&PersistentSymbolKey::TypeDescriptor(exact_type));
    let mut name = Vec::with_capacity(symbol.as_str().len() + 1);
    name.push(b'_');
    name.extend_from_slice(symbol.as_str().as_bytes());
    name
}

fn itable_directory_failure(
    plan: &StrongTypeRegistrationPlanV1,
    entry: Option<usize>,
    kind: TypeDescriptorITableDirectoryFailureV1,
) -> StrongTypeRegistrationValidationError {
    StrongTypeRegistrationValidationError::DescriptorITableDirectoryMismatch {
        exact_type: plan.exact_type(),
        entry,
        kind,
    }
}

fn itable_directory_error<T>(
    plan: &StrongTypeRegistrationPlanV1,
    entry: Option<usize>,
    kind: TypeDescriptorITableDirectoryFailureV1,
) -> Result<T, StrongTypeRegistrationValidationError> {
    Err(itable_directory_failure(plan, entry, kind))
}

fn verify_descriptor_diagnostic_relocation(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongTypeRegistrationPlanV1,
    diagnostic_section: std::num::NonZeroU8,
    diagnostic_value: u64,
) -> Result<VerifiedRelocationUseV1, StrongTypeRegistrationValidationError> {
    use TypeDescriptorDiagnosticRelocationFailureV1 as Failure;

    let relocations = member
        .relocations()
        .iter()
        .filter(|relocation| {
            relocation.containing_atom() == plan.descriptor_primary_atom()
                && relocation.offset_within_atom() == TYPE_DESCRIPTOR_DIAGNOSTIC_POINTER_OFFSET
        })
        .collect::<Vec<_>>();
    if relocations.len() != 1 {
        return descriptor_diagnostic_relocation_error(plan.exact_type(), Failure::Count);
    }
    let relocation = relocations[0];
    let kind = if relocation.containing_atom_role() != DefinitionAtomRole::Primary {
        Some(Failure::ContainingAtomRole)
    } else if relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(Failure::SectionRole)
    } else if relocation.width_bytes() != 8 {
        Some(Failure::Width)
    } else if relocation.encoded_value() != 0 {
        Some(Failure::EncodedValue)
    } else {
        match relocation.shape() {
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                target:
                    VerifiedRelocationTargetV1::LocalDefinition {
                        owner_atom,
                        section_ordinal,
                        value,
                        ..
                    },
            } if *owner_atom != Some(plan.diagnostic_atom()) => Some(Failure::TargetAtom),
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                target:
                    VerifiedRelocationTargetV1::LocalDefinition {
                        section_ordinal, ..
                    },
            } if *section_ordinal != diagnostic_section => Some(Failure::TargetSection),
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                target: VerifiedRelocationTargetV1::LocalDefinition { value, .. },
            } if *value != diagnostic_value => Some(Failure::TargetValue),
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                target: VerifiedRelocationTargetV1::LocalDefinition { .. },
            } => None,
            VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { .. } => Some(Failure::TargetKind),
            _ => Some(Failure::Form),
        }
    };
    if let Some(kind) = kind {
        return descriptor_diagnostic_relocation_error(plan.exact_type(), kind);
    }
    Ok(relocation.clone())
}

fn descriptor_diagnostic_relocation_error<T>(
    exact_type: PersistentExactTypeId,
    kind: TypeDescriptorDiagnosticRelocationFailureV1,
) -> Result<T, StrongTypeRegistrationValidationError> {
    Err(
        StrongTypeRegistrationValidationError::DescriptorDiagnosticRelocationMismatch {
            exact_type,
            kind,
        },
    )
}

fn required_scoop_member(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: &StrongTypeRegistrationPlanV1,
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
    plan: &StrongTypeRegistrationPlanV1,
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
    plan: &StrongTypeRegistrationPlanV1,
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
    plan: &StrongTypeRegistrationPlanV1,
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
    plan: &StrongTypeRegistrationPlanV1,
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
