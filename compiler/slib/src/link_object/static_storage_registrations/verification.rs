use std::collections::BTreeMap;

use scoop_identity::{
    DefinitionAtomRole, DigestPatchIntentId, DigestSemanticFieldRole, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, PersistentStaticStorageId,
};
use scoop_lir::{
    RefScan, StaticStorageRelocationTableArtifactV1, StrongStaticStorageInitialArtifactPlanV1,
    StrongStaticStorageRegistrationPlanSetV1, StrongStaticStorageRegistrationPlanV1,
};

use super::physical::{
    StaticStorageAtomRangeV1, atom_file_range, atom_range, validate_objects, verified_member,
};
use super::record::{DESCRIPTOR_SIZE, validate_record_bytes};
use super::relocations::{
    VerifiedStaticStorageRelocations, sentinel_target_key, verify_relocations,
};
use super::{
    StaticStorageArtifactRoleV1, StaticStorageRegistrationPatchFailureV1,
    StrongStaticStorageRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1, StrongRelocationBindingV1,
    VerifiedDefinitionAtomRangeV1, VerifiedMaterializedPatchSiteV1, VerifiedRelocationUseV1,
    VerifiedScoopLirDigestPatchSiteSetV1,
};

const SCAN_FINGERPRINT_OFFSET: u64 = 168;
const LAYOUT_FINGERPRINT_OFFSET: u64 = 200;
const DIGEST_WIDTH: u8 = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageRegistrationV1 {
    storage: PersistentStaticStorageId,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    storage_member: SlibMemberId,
    storage_relocation: StrongRelocationBindingV1,
    scan_relocation: StrongRelocationBindingV1,
    template_relocation: VerifiedRelocationUseV1,
    relocation_table_relocation: VerifiedRelocationUseV1,
    initial_storage_relocations: Vec<StrongRelocationBindingV1>,
    initial_target_relocations: Vec<StrongRelocationBindingV1>,

    scan_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
    layout_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
}

impl VerifiedStrongStaticStorageRegistrationV1 {
    pub const fn storage(&self) -> PersistentStaticStorageId {
        self.storage
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

    pub const fn storage_member(&self) -> SlibMemberId {
        self.storage_member
    }

    pub const fn storage_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.storage_relocation
    }

    pub const fn scan_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.scan_relocation
    }

    pub const fn template_relocation(&self) -> &VerifiedRelocationUseV1 {
        &self.template_relocation
    }

    pub const fn relocation_table_relocation(&self) -> &VerifiedRelocationUseV1 {
        &self.relocation_table_relocation
    }

    pub fn initial_storage_relocations(&self) -> &[StrongRelocationBindingV1] {
        &self.initial_storage_relocations
    }

    pub fn initial_target_relocations(&self) -> &[StrongRelocationBindingV1] {
        &self.initial_target_relocations
    }

    pub const fn scan_fingerprint_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.scan_fingerprint_patch
    }

    pub const fn layout_fingerprint_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.layout_fingerprint_patch
    }
}

/// Proof that every final-LIR static storage has one exact provisional root
/// registration and that all of its runtime-facing artifacts are materialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageRegistrationSetV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongStaticStorageRegistrationPlanSetV1,
    registrations: Vec<VerifiedStrongStaticStorageRegistrationV1>,
}

impl VerifiedStrongStaticStorageRegistrationSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &StrongStaticStorageRegistrationPlanSetV1 {
        &self.plan
    }

    pub fn registrations(&self) -> &[VerifiedStrongStaticStorageRegistrationV1] {
        &self.registrations
    }
}

pub fn verify_strong_static_storage_registrations_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongStaticStorageRegistrationPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongStaticStorageRegistrationSetV1,
    StrongStaticStorageRegistrationValidationError,
> {
    if patch_sites.producer() != plan.producer() {
        return Err(StrongStaticStorageRegistrationValidationError::DigestPatchProducerMismatch);
    }
    let objects = validate_objects(patch_sites.builtins(), scoop_objects)?;
    let mut registrations = Vec::with_capacity(plan.registrations().len());
    for registration in plan.registrations() {
        registrations.push(verify_registration(&patch_sites, &objects, registration)?);
    }
    validate_shared_sentinels(&patch_sites, &plan, &registrations)?;
    Ok(VerifiedStrongStaticStorageRegistrationSetV1 {
        patch_sites,
        plan,
        registrations,
    })
}

fn validate_shared_sentinels(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongStaticStorageRegistrationPlanSetV1,
    verified: &[VerifiedStrongStaticStorageRegistrationV1],
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let mut template_target = None;
    let mut relocation_target = None;
    for (plan, verified) in plan.registrations().iter().zip(verified) {
        let member = verified_member(patch_sites.builtins(), verified.member())?;
        if matches!(
            plan.initial_artifacts(),
            StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
        ) {
            insert_shared_target(
                &mut template_target,
                sentinel_target_key(member, verified.template_relocation()),
                StaticStorageArtifactRoleV1::InitialTemplate,
            )?;
        }
        if matches!(
            plan.initial_artifacts(),
            StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
                | StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
                    relocation_table: StaticStorageRelocationTableArtifactV1::SharedEmptySentinel,
                    ..
                }
        ) {
            insert_shared_target(
                &mut relocation_target,
                sentinel_target_key(member, verified.relocation_table_relocation()),
                StaticStorageArtifactRoleV1::InitialRelocationTable,
            )?;
        }
    }
    if template_target.is_some() && template_target == relocation_target {
        return Err(StrongStaticStorageRegistrationValidationError::SentinelTargetCollision);
    }
    Ok(())
}

fn insert_shared_target(
    shared: &mut Option<(SlibMemberId, u32, u64)>,
    actual: Option<(SlibMemberId, u32, u64)>,
    role: StaticStorageArtifactRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let actual = actual
        .ok_or(StrongStaticStorageRegistrationValidationError::SentinelTargetMismatch(role))?;
    if let Some(expected) = shared {
        if *expected != actual {
            return Err(
                StrongStaticStorageRegistrationValidationError::SentinelTargetMismatch(role),
            );
        }
    } else {
        *shared = Some(actual);
    }
    Ok(())
}

fn verify_registration(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> Result<VerifiedStrongStaticStorageRegistrationV1, StrongStaticStorageRegistrationValidationError>
{
    let builtins = patch_sites.builtins();
    let member = required_scoop_member(builtins, plan, plan.registration_definition_plan())?;
    let verified_member = verified_member(builtins, member)?;
    let (primary_symbol_table_index, file_start, file_end) = require_primary_atom(
        verified_member,
        plan,
        plan.registration_definition_plan(),
        plan.registration_primary_atom(),
        StaticStorageArtifactRoleV1::Registration,
        BuiltinObjectSectionRoleV1::ReadOnlyData,
    )?;
    require_size(
        plan,
        StaticStorageArtifactRoleV1::Registration,
        DESCRIPTOR_SIZE as u64,
        file_end - file_start,
    )?;

    let storage_artifacts = verify_storage_artifacts(patch_sites, objects, plan)?;
    let relocations = verify_relocations(patch_sites, verified_member, plan)?;

    let scan_fingerprint_patch = require_patch(
        patch_sites,
        plan,
        member,
        file_start,
        plan.scan_fingerprint_patch(),
        plan.scan_fingerprint_node(),
        DigestSemanticFieldRole::Scan,
        SCAN_FINGERPRINT_OFFSET,
    )?;
    let layout_fingerprint_patch = require_patch(
        patch_sites,
        plan,
        member,
        file_start,
        plan.layout_fingerprint_patch(),
        plan.layout_fingerprint_node(),
        DigestSemanticFieldRole::Layout,
        LAYOUT_FINGERPRINT_OFFSET,
    )?;
    validate_exact_atom_patch_set(patch_sites, plan, member)?;
    validate_record_bytes(
        objects[&member],
        file_start,
        plan,
        relocations.template.encoded_value(),
        relocations.relocation_table.encoded_value(),
    )?;

    Ok(build_verified_registration(
        plan,
        member,
        primary_symbol_table_index,
        file_start,
        storage_artifacts,
        relocations,
        scan_fingerprint_patch,
        layout_fingerprint_patch,
    ))
}

fn verify_storage_artifacts(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> Result<SlibMemberId, StrongStaticStorageRegistrationValidationError> {
    let builtins = patch_sites.builtins();
    let storage_member = required_scoop_member(builtins, plan, plan.storage_definition_plan())?;
    let storage_index = verified_member(builtins, storage_member)?;
    let expected_storage_section = match plan.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {
            BuiltinObjectSectionRoleV1::ZeroFill
        }
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue { .. } => {
            BuiltinObjectSectionRoleV1::WritableData
        }
    };
    let (_, storage_range) = require_storage_primary_atom(
        storage_index,
        plan,
        plan.storage_definition_plan(),
        plan.storage_primary_atom(),
        expected_storage_section,
    )?;
    require_size(
        plan,
        StaticStorageArtifactRoleV1::Storage,
        plan.semantic().allocation_extent(),
        storage_range.byte_size(),
    )?;

    if plan.semantic().value_layout().local().is_some() {
        require_primary_atom(
            verified_member(
                builtins,
                required_scoop_member(builtins, plan, plan.layout_definition_plan())?,
            )?,
            plan,
            plan.layout_definition_plan(),
            plan.layout_primary_atom(),
            StaticStorageArtifactRoleV1::Layout,
            BuiltinObjectSectionRoleV1::ReadOnlyData,
        )?;
        let scan_member = required_scoop_member(builtins, plan, plan.scan_definition_plan())?;
        let scan_index = verified_member(builtins, scan_member)?;
        let (_, scan_start, scan_end) = require_primary_atom(
            scan_index,
            plan,
            plan.scan_definition_plan(),
            plan.scan_primary_atom(),
            StaticStorageArtifactRoleV1::ScanProgram,
            BuiltinObjectSectionRoleV1::ReadOnlyData,
        )?;
        let expected_scan = canonical_scan_bytes(plan);
        require_size(
            plan,
            StaticStorageArtifactRoleV1::ScanProgram,
            u64::try_from(expected_scan.len()).unwrap(),
            scan_end - scan_start,
        )?;
        validate_artifact_bytes(
            objects[&scan_member],
            scan_start,
            &expected_scan,
            plan,
            StaticStorageArtifactRoleV1::ScanProgram,
        )?;
    }

    match plan.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {}
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            template_atom,
            relocation_table,
        } => {
            let storage_start = match storage_range {
                StaticStorageAtomRangeV1::FileBacked { start, .. } => start,
                StaticStorageAtomRangeV1::ZeroFill { .. } => {
                    return Err(
                        StrongStaticStorageRegistrationValidationError::AtomSectionMismatch {
                            storage: plan.semantic().storage(),
                            role: StaticStorageArtifactRoleV1::Storage,
                        },
                    );
                }
            };
            let (_, template_start, template_end) = require_associated_atom(
                storage_index,
                plan,
                template_atom,
                DefinitionAtomRole::AddressTakenConstant,
                StaticStorageArtifactRoleV1::InitialTemplate,
            )?;
            let template = plan.semantic().initial_state().initial_template();
            require_size(
                plan,
                StaticStorageArtifactRoleV1::InitialTemplate,
                u64::try_from(template.len()).unwrap(),
                template_end - template_start,
            )?;
            validate_artifact_bytes(
                objects[&storage_member],
                template_start,
                template,
                plan,
                StaticStorageArtifactRoleV1::InitialTemplate,
            )?;
            validate_artifact_bytes(
                objects[&storage_member],
                storage_start,
                template,
                plan,
                StaticStorageArtifactRoleV1::Storage,
            )?;

            match relocation_table {
                StaticStorageRelocationTableArtifactV1::SharedEmptySentinel => {}
                StaticStorageRelocationTableArtifactV1::Defined { atom } => {
                    let (_, table_start, table_end) = require_associated_atom(
                        storage_index,
                        plan,
                        atom,
                        DefinitionAtomRole::RuntimeRecord,
                        StaticStorageArtifactRoleV1::InitialRelocationTable,
                    )?;
                    let relocations = plan.semantic().initial_state().immortal_relocations();
                    require_size(
                        plan,
                        StaticStorageArtifactRoleV1::InitialRelocationTable,
                        u64::try_from(relocations.len()).unwrap() * 16,
                        table_end - table_start,
                    )?;
                    let mut expected = Vec::with_capacity(relocations.len() * 16);
                    for relocation in relocations {
                        expected.extend_from_slice(&relocation.pointer_offset().to_le_bytes());
                        expected.extend_from_slice(&[0; 8]);
                    }
                    validate_artifact_bytes(
                        objects[&storage_member],
                        table_start,
                        &expected,
                        plan,
                        StaticStorageArtifactRoleV1::InitialRelocationTable,
                    )?;
                }
            }
        }
    }
    Ok(storage_member)
}

#[allow(clippy::too_many_arguments)]
fn build_verified_registration(
    plan: &StrongStaticStorageRegistrationPlanV1,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    storage_artifacts: SlibMemberId,
    relocations: VerifiedStaticStorageRelocations,

    scan_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
    layout_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
) -> VerifiedStrongStaticStorageRegistrationV1 {
    VerifiedStrongStaticStorageRegistrationV1 {
        storage: plan.semantic().storage(),
        member,
        primary_symbol_table_index,
        checked_offset,
        storage_member: storage_artifacts,
        storage_relocation: relocations.storage,
        scan_relocation: relocations.scan,
        template_relocation: relocations.template,
        relocation_table_relocation: relocations.relocation_table,
        initial_storage_relocations: relocations.initial_storage,
        initial_target_relocations: relocations.initial_targets,

        scan_fingerprint_patch,
        layout_fingerprint_patch,
    }
}

pub(super) fn required_scoop_member(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    definition: ObjectDefinitionPlanId,
) -> Result<SlibMemberId, StrongStaticStorageRegistrationValidationError> {
    let member = builtins
        .member_plan()
        .member_for_definition(definition)
        .ok_or(
            StrongStaticStorageRegistrationValidationError::MissingDefinitionAssignment {
                storage: plan.semantic().storage(),
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
            StrongStaticStorageRegistrationValidationError::DefinitionAssignedToNonScoopMember {
                storage: plan.semantic().storage(),
                member,
            },
        );
    }
    Ok(member)
}

#[allow(clippy::too_many_arguments)]
fn require_primary_atom(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    definition_id: ObjectDefinitionPlanId,
    atom_id: ObjectDefinitionAtomId,
    role: StaticStorageArtifactRoleV1,
    expected_section: BuiltinObjectSectionRoleV1,
) -> Result<(u32, u64, u64), StrongStaticStorageRegistrationValidationError> {
    let (primary_symbol_table_index, atom) =
        primary_atom(member, plan, definition_id, atom_id, role)?;
    let (section, start, end) = atom_file_range(member, atom).map_err(|kind| {
        StrongStaticStorageRegistrationValidationError::InvalidAtomRange {
            storage: plan.semantic().storage(),
            role,
            atom: atom_id,
            kind,
        }
    })?;
    if section != expected_section {
        return Err(
            StrongStaticStorageRegistrationValidationError::AtomSectionMismatch {
                storage: plan.semantic().storage(),
                role,
            },
        );
    }
    require_primary_alignment(plan, role, atom.start())?;
    Ok((primary_symbol_table_index, start, end))
}

fn require_storage_primary_atom(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    definition_id: ObjectDefinitionPlanId,
    atom_id: ObjectDefinitionAtomId,
    expected_section: BuiltinObjectSectionRoleV1,
) -> Result<(u32, StaticStorageAtomRangeV1), StrongStaticStorageRegistrationValidationError> {
    let role = StaticStorageArtifactRoleV1::Storage;
    let (primary_symbol_table_index, atom) =
        primary_atom(member, plan, definition_id, atom_id, role)?;
    let range = atom_range(member, atom).map_err(|kind| {
        StrongStaticStorageRegistrationValidationError::InvalidAtomRange {
            storage: plan.semantic().storage(),
            role,
            atom: atom_id,
            kind,
        }
    })?;
    if range.role() != expected_section {
        return Err(
            StrongStaticStorageRegistrationValidationError::AtomSectionMismatch {
                storage: plan.semantic().storage(),
                role,
            },
        );
    }
    require_primary_alignment(plan, role, atom.start())?;
    Ok((primary_symbol_table_index, range))
}

fn primary_atom(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    definition_id: ObjectDefinitionPlanId,
    atom_id: ObjectDefinitionAtomId,
    role: StaticStorageArtifactRoleV1,
) -> Result<(u32, VerifiedDefinitionAtomRangeV1), StrongStaticStorageRegistrationValidationError> {
    let definition = member.definitions().definition(definition_id).ok_or(
        StrongStaticStorageRegistrationValidationError::MissingVerifiedDefinition {
            storage: plan.semantic().storage(),
            definition: definition_id,
        },
    )?;
    if definition.primary_atom() != atom_id {
        return Err(
            StrongStaticStorageRegistrationValidationError::PrimaryAtomMismatch {
                storage: plan.semantic().storage(),
                role,
                expected: atom_id,
                actual: definition.primary_atom(),
            },
        );
    }
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| atom.atom() == atom_id && atom.atom_role() == DefinitionAtomRole::Primary)
        .copied()
        .ok_or(
            StrongStaticStorageRegistrationValidationError::MissingAtom {
                storage: plan.semantic().storage(),
                role,
                atom: atom_id,
            },
        )?;
    Ok((definition.primary_symbol_table_index(), atom))
}

fn require_primary_alignment(
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageArtifactRoleV1,
    address: u64,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let required_alignment = match role {
        StaticStorageArtifactRoleV1::Storage => plan.semantic().required_alignment(),
        StaticStorageArtifactRoleV1::Registration
        | StaticStorageArtifactRoleV1::Layout
        | StaticStorageArtifactRoleV1::ScanProgram => 8,
        StaticStorageArtifactRoleV1::InitialTemplate
        | StaticStorageArtifactRoleV1::InitialRelocationTable => {
            unreachable!("associated static-storage atoms use require_associated_atom")
        }
    };
    require_alignment(plan, role, required_alignment, address)
}

fn require_associated_atom(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    atom_id: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    role: StaticStorageArtifactRoleV1,
) -> Result<(BuiltinObjectSectionRoleV1, u64, u64), StrongStaticStorageRegistrationValidationError>
{
    let definition = member
        .definitions()
        .definition(plan.storage_definition_plan())
        .ok_or(
            StrongStaticStorageRegistrationValidationError::MissingVerifiedDefinition {
                storage: plan.semantic().storage(),
                definition: plan.storage_definition_plan(),
            },
        )?;
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| atom.atom() == atom_id && atom.atom_role() == atom_role)
        .copied()
        .ok_or(
            StrongStaticStorageRegistrationValidationError::MissingAtom {
                storage: plan.semantic().storage(),
                role,
                atom: atom_id,
            },
        )?;
    let range = atom_file_range(member, atom).map_err(|kind| {
        StrongStaticStorageRegistrationValidationError::InvalidAtomRange {
            storage: plan.semantic().storage(),
            role,
            atom: atom_id,
            kind,
        }
    })?;
    if range.0 != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongStaticStorageRegistrationValidationError::AtomSectionMismatch {
                storage: plan.semantic().storage(),
                role,
            },
        );
    }
    let required_alignment = match role {
        StaticStorageArtifactRoleV1::InitialTemplate => 1,
        StaticStorageArtifactRoleV1::InitialRelocationTable => 8,
        _ => unreachable!("only static initial-state support uses associated atoms"),
    };
    require_alignment(plan, role, required_alignment, atom.start())?;
    Ok(range)
}

fn require_alignment(
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageArtifactRoleV1,
    required: u64,
    address: u64,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    if required == 0 || !required.is_power_of_two() || address % required != 0 {
        return Err(
            StrongStaticStorageRegistrationValidationError::AtomAlignmentMismatch {
                storage: plan.semantic().storage(),
                role,
                required,
                address,
            },
        );
    }
    Ok(())
}

fn require_size(
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageArtifactRoleV1,
    expected: u64,
    actual: u64,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    if actual != expected {
        return Err(
            StrongStaticStorageRegistrationValidationError::AtomSizeMismatch {
                storage: plan.semantic().storage(),
                role,
                expected,
                actual,
            },
        );
    }
    Ok(())
}

fn validate_artifact_bytes(
    object: &[u8],
    file_start: u64,
    expected: &[u8],
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageArtifactRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongStaticStorageRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().storage(),
        )
    })?;
    let end = start.checked_add(expected.len()).ok_or(
        StrongStaticStorageRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().storage(),
        ),
    )?;
    let actual = object.get(start..end).ok_or(
        StrongStaticStorageRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().storage(),
        ),
    )?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(
            StrongStaticStorageRegistrationValidationError::ArtifactByteMismatch {
                storage: plan.semantic().storage(),
                role,
                offset_within_atom: u64::try_from(offset).unwrap(),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

fn canonical_scan_bytes(plan: &StrongStaticStorageRegistrationPlanV1) -> Vec<u8> {
    let mut bytes = Vec::new();
    match plan.semantic().scan_program() {
        RefScan::None => bytes.extend_from_slice(&0_u64.to_le_bytes()),
        RefScan::References(offsets) => {
            bytes.extend_from_slice(&u64::try_from(offsets.len()).unwrap().to_le_bytes());
            for offset in offsets {
                bytes.extend_from_slice(&offset.to_le_bytes());
            }
        }
        RefScan::Sequence(_) | RefScan::Array { .. } => {
            unreachable!("semantic plan rejects non-value scans")
        }
    }
    bytes
}

#[allow(clippy::too_many_arguments)]
fn require_patch(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    member: SlibMemberId,
    file_start: u64,
    intent: DigestPatchIntentId,
    source: scoop_identity::DigestNodeId,
    field: DigestSemanticFieldRole,
    offset_within_atom: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, StrongStaticStorageRegistrationValidationError> {
    let patch = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == intent)
        .copied()
        .ok_or(
            StrongStaticStorageRegistrationValidationError::MissingPatch {
                storage: plan.semantic().storage(),
                intent,
            },
        )?;
    let expected_checked_offset = file_start.checked_add(offset_within_atom).ok_or(
        StrongStaticStorageRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().storage(),
        ),
    )?;
    use StaticStorageRegistrationPatchFailureV1 as Failure;
    let failure = if patch.source() != source {
        Some(Failure::Source)
    } else if patch.semantic_field_role() != field {
        Some(Failure::SemanticFieldRole)
    } else if patch.member() != member {
        Some(Failure::Member)
    } else if patch.definition() != plan.registration_definition_plan() {
        Some(Failure::Definition)
    } else if patch.atom() != plan.registration_primary_atom() {
        Some(Failure::Atom)
    } else if patch.atom_role() != DefinitionAtomRole::Primary {
        Some(Failure::AtomRole)
    } else if patch.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(Failure::SectionRole)
    } else if patch.offset_within_atom() != offset_within_atom {
        Some(Failure::OffsetWithinAtom)
    } else if patch.checked_offset() != expected_checked_offset {
        Some(Failure::CheckedOffset)
    } else if patch.width_bytes() != DIGEST_WIDTH {
        Some(Failure::Width)
    } else {
        None
    };
    if let Some(kind) = failure {
        return Err(
            StrongStaticStorageRegistrationValidationError::PatchMismatch {
                storage: plan.semantic().storage(),
                intent,
                kind,
            },
        );
    }
    Ok(patch)
}

fn validate_exact_atom_patch_set(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    member: SlibMemberId,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let expected = [
        plan.scan_fingerprint_patch(),
        plan.layout_fingerprint_patch(),
    ];
    if let Some(site) = patch_sites.sites().iter().find(|site| {
        site.member() == member
            && site.atom() == plan.registration_primary_atom()
            && !expected.contains(&site.intent())
    }) {
        return Err(
            StrongStaticStorageRegistrationValidationError::UnexpectedPatchInPrimaryAtom {
                storage: plan.semantic().storage(),
                intent: site.intent(),
            },
        );
    }
    Ok(())
}
