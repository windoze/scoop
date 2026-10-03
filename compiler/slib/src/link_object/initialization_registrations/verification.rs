use std::collections::BTreeMap;

use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestPatchIntentId, DigestSemanticFieldRole,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentInitializationUnitId,
};
use scoop_lir::{
    StrongInitializationDependencyRefV2, StrongInitializationRegistrationSchedulePlanV1,
    StrongInitializationUnitRegistrationPlan, StrongInitializationUnitRegistrationPlanSet,
    StrongInitializationUnitRegistrationPlanSetV1,
};

use super::digest::validate_digest_graph;
use super::physical::{atom_file_range, validate_objects, verified_member};
use super::record::{CELL_SIZE, DESCRIPTOR_SIZE, validate_cell_bytes, validate_record_bytes};
use super::relocations::{VerifiedInitializationRelocationsV1, verify_relocations};
use super::{
    InitializationArtifactRoleV1, InitializationRegistrationPatchFailureV1,
    InitializationRelocationFailureV1, InitializationRelocationRoleV1,
    StrongInitializationRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1, StrongRelocationBindingV1,
    VerifiedMaterializedPatchSiteV1, VerifiedMemberObjectRelocationIndexV1,
    VerifiedRelocationUseV1, VerifiedScoopLirDigestPatchSiteSetV1,
};

const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const GATEWAY_DEFINITION_FINGERPRINT_OFFSET: u64 = 312;
const DIGEST_WIDTH: u8 = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationRegistrationV1 {
    unit: PersistentInitializationUnitId,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    cell_member: SlibMemberId,
    cell_primary_symbol_table_index: u32,
    cell_checked_offset: u64,
    diagnostic_checked_offset: u64,
    registration_diagnostic_relocation: VerifiedRelocationUseV1,
    registration_cell_relocation: StrongRelocationBindingV1,
    registration_storage_relocation: StrongRelocationBindingV1,
    registration_failure_relocation: StrongRelocationBindingV1,
    registration_initializer_relocation: StrongRelocationBindingV1,
    registration_ensure_relocation: StrongRelocationBindingV1,
    registration_gateway_relocation: Option<StrongRelocationBindingV1>,
    registration_definition_patch: VerifiedMaterializedPatchSiteV1,
    gateway_definition_patch: Option<VerifiedMaterializedPatchSiteV1>,
}

impl VerifiedStrongInitializationRegistrationV1 {
    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
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

    pub const fn cell_member(&self) -> SlibMemberId {
        self.cell_member
    }

    pub const fn cell_primary_symbol_table_index(&self) -> u32 {
        self.cell_primary_symbol_table_index
    }

    pub const fn cell_checked_offset(&self) -> u64 {
        self.cell_checked_offset
    }

    pub const fn diagnostic_checked_offset(&self) -> u64 {
        self.diagnostic_checked_offset
    }

    pub const fn registration_diagnostic_relocation(&self) -> &VerifiedRelocationUseV1 {
        &self.registration_diagnostic_relocation
    }

    pub const fn registration_cell_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.registration_cell_relocation
    }

    pub const fn registration_storage_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.registration_storage_relocation
    }

    pub const fn registration_failure_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.registration_failure_relocation
    }

    pub const fn registration_initializer_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.registration_initializer_relocation
    }

    pub const fn registration_ensure_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.registration_ensure_relocation
    }

    pub const fn registration_gateway_relocation(&self) -> Option<&StrongRelocationBindingV1> {
        self.registration_gateway_relocation.as_ref()
    }

    pub const fn registration_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.registration_definition_patch
    }

    pub const fn gateway_definition_patch(&self) -> Option<VerifiedMaterializedPatchSiteV1> {
        self.gateway_definition_patch
    }
}

/// Proof that every final-LIR initialization unit has exactly one canonical
/// cell and provisional initialization registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongInitializationRegistrationSetV1<D = PersistentInitializationUnitId> {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongInitializationUnitRegistrationPlanSet<D>,
    registrations: Vec<VerifiedStrongInitializationRegistrationV1>,
}

pub type VerifiedStrongInitializationRegistrationSetV2 =
    VerifiedStrongInitializationRegistrationSetV1<StrongInitializationDependencyRefV2>;

impl<D> VerifiedStrongInitializationRegistrationSetV1<D> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &StrongInitializationUnitRegistrationPlanSet<D> {
        &self.plan
    }

    pub fn registrations(&self) -> &[VerifiedStrongInitializationRegistrationV1] {
        &self.registrations
    }
}

pub fn verify_strong_initialization_registrations_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongInitializationUnitRegistrationPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationRegistrationSetV1,
    StrongInitializationRegistrationValidationError,
> {
    verify_strong_initialization_registrations(patch_sites, plan, scoop_objects)
}

pub fn verify_strong_initialization_registrations_v2(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: scoop_lir::StrongInitializationUnitRegistrationPlanSetV2,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationRegistrationSetV2,
    StrongInitializationRegistrationValidationError,
> {
    verify_strong_initialization_registrations(patch_sites, plan, scoop_objects)
}

fn verify_strong_initialization_registrations<D>(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongInitializationUnitRegistrationPlanSet<D>,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<
    VerifiedStrongInitializationRegistrationSetV1<D>,
    StrongInitializationRegistrationValidationError,
>
where
    D: scoop_lir::StrongInitializationDependencyReference,
{
    if patch_sites.producer() != plan.producer() {
        return Err(StrongInitializationRegistrationValidationError::DigestPatchProducerMismatch);
    }
    let objects = validate_objects(patch_sites.builtins(), scoop_objects)?;
    let mut registrations = Vec::with_capacity(plan.registrations().len());
    for registration in plan.registrations() {
        validate_digest_graph(patch_sites.digest_plan(), registration)?;
        registrations.push(verify_registration(&patch_sites, &objects, registration)?);
    }
    Ok(VerifiedStrongInitializationRegistrationSetV1 {
        patch_sites,
        plan,
        registrations,
    })
}

fn verify_registration<D>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<
    VerifiedStrongInitializationRegistrationV1,
    StrongInitializationRegistrationValidationError,
>
where
    D: scoop_lir::StrongInitializationDependencyReference,
{
    let builtins = patch_sites.builtins();
    let cell_member = required_scoop_member(builtins, plan, plan.cell_definition_plan())?;
    let cell_index = verified_member(builtins, cell_member)?;
    let (cell_primary_symbol_table_index, cell_start, cell_end) = require_primary_atom(
        cell_index,
        plan,
        plan.cell_definition_plan(),
        plan.cell_primary_atom(),
        InitializationArtifactRoleV1::Cell,
        BuiltinObjectSectionRoleV1::WritableData,
        CELL_SIZE as u64,
    )?;
    if cell_index
        .relocations()
        .iter()
        .any(|relocation| relocation.containing_atom() == plan.cell_primary_atom())
    {
        return Err(
            StrongInitializationRegistrationValidationError::RelocationMismatch {
                unit: plan.semantic().unit(),
                role: InitializationRelocationRoleV1::RegistrationCell,
                kind: InitializationRelocationFailureV1::Count,
            },
        );
    }
    validate_cell_bytes(objects[&cell_member], cell_start, plan)?;
    debug_assert_eq!(cell_end - cell_start, CELL_SIZE as u64);

    let member = required_scoop_member(builtins, plan, plan.registration_definition_plan())?;
    let registration_index = verified_member(builtins, member)?;
    let (primary_symbol_table_index, checked_offset, registration_end) = require_primary_atom(
        registration_index,
        plan,
        plan.registration_definition_plan(),
        plan.registration_primary_atom(),
        InitializationArtifactRoleV1::Registration,
        BuiltinObjectSectionRoleV1::ReadOnlyData,
        DESCRIPTOR_SIZE as u64,
    )?;
    let diagnostic_checked_offset = require_diagnostic_atom(registration_index, plan)?;
    validate_record_bytes(objects[&member], checked_offset, plan)?;
    debug_assert_eq!(registration_end - checked_offset, DESCRIPTOR_SIZE as u64);

    let relocations = verify_relocations(patch_sites, objects, registration_index, plan)?;
    let registration_definition_patch = require_patch(
        patch_sites,
        plan,
        plan.registration_definition_patch(),
        plan.registration_fingerprint_node(),
        DigestSemanticFieldRole::RegistrationDefinition,
        member,
        checked_offset,
        REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
    )?;
    let gateway_definition_patch = match plan.schedule() {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup {
            gateway,
            gateway_definition_patch,
        } => Some(require_patch(
            patch_sites,
            plan,
            *gateway_definition_patch,
            gateway.body_definition_node(),
            DigestSemanticFieldRole::GatewayDefinition,
            member,
            checked_offset,
            GATEWAY_DEFINITION_FINGERPRINT_OFFSET,
        )?),
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => None,
    };
    validate_exact_atom_patch_set(patch_sites, plan, member)?;

    Ok(build_verified(
        plan,
        member,
        primary_symbol_table_index,
        checked_offset,
        cell_member,
        cell_primary_symbol_table_index,
        cell_start,
        diagnostic_checked_offset,
        relocations,
        registration_definition_patch,
        gateway_definition_patch,
    ))
}

fn require_diagnostic_atom<D>(
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<u64, StrongInitializationRegistrationValidationError> {
    let definition = member
        .definitions()
        .definition(plan.registration_definition_plan())
        .expect("descriptor primary validation already proved the definition");
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| {
            atom.atom() == plan.diagnostic_atom()
                && atom.atom_role() == DefinitionAtomRole::AddressTakenConstant
        })
        .copied()
        .ok_or(
            StrongInitializationRegistrationValidationError::MissingDiagnosticAtom {
                unit: plan.semantic().unit(),
                atom: plan.diagnostic_atom(),
            },
        )?;
    let (section, start, end) = atom_file_range(member, atom).map_err(|kind| {
        StrongInitializationRegistrationValidationError::InvalidPrimaryAtomFileRange {
            unit: plan.semantic().unit(),
            role: InitializationArtifactRoleV1::DiagnosticBytes,
            atom: plan.diagnostic_atom(),
            kind,
        }
    })?;
    if section != BuiltinObjectSectionRoleV1::CString {
        return Err(
            StrongInitializationRegistrationValidationError::PrimaryAtomSectionMismatch {
                unit: plan.semantic().unit(),
                role: InitializationArtifactRoleV1::DiagnosticBytes,
            },
        );
    }
    let expected_size = u64::try_from(plan.semantic().diagnostic_path().len()).unwrap() + 1;
    if end - start != expected_size {
        return Err(
            StrongInitializationRegistrationValidationError::PrimaryAtomSizeMismatch {
                unit: plan.semantic().unit(),
                role: InitializationArtifactRoleV1::DiagnosticBytes,
                expected: expected_size,
                actual: end - start,
            },
        );
    }
    Ok(start)
}

#[allow(clippy::too_many_arguments)]
fn build_verified<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    cell_member: SlibMemberId,
    cell_primary_symbol_table_index: u32,
    cell_checked_offset: u64,
    diagnostic_checked_offset: u64,
    relocations: VerifiedInitializationRelocationsV1,
    registration_definition_patch: VerifiedMaterializedPatchSiteV1,
    gateway_definition_patch: Option<VerifiedMaterializedPatchSiteV1>,
) -> VerifiedStrongInitializationRegistrationV1 {
    VerifiedStrongInitializationRegistrationV1 {
        unit: plan.semantic().unit(),
        member,
        primary_symbol_table_index,
        checked_offset,
        cell_member,
        cell_primary_symbol_table_index,
        cell_checked_offset,
        diagnostic_checked_offset,
        registration_diagnostic_relocation: relocations.registration_diagnostic,
        registration_cell_relocation: relocations.registration_cell,
        registration_storage_relocation: relocations.registration_storage,
        registration_failure_relocation: relocations.registration_failure,
        registration_initializer_relocation: relocations.registration_initializer,
        registration_ensure_relocation: relocations.registration_ensure,
        registration_gateway_relocation: relocations.registration_gateway,
        registration_definition_patch,
        gateway_definition_patch,
    }
}

pub(super) fn required_scoop_member<D>(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    definition: ObjectDefinitionPlanId,
) -> Result<SlibMemberId, StrongInitializationRegistrationValidationError> {
    let member = builtins
        .member_plan()
        .member_for_definition(definition)
        .ok_or(
            StrongInitializationRegistrationValidationError::MissingDefinitionAssignment {
                unit: plan.semantic().unit(),
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
            StrongInitializationRegistrationValidationError::DefinitionAssignedToNonScoopMember {
                unit: plan.semantic().unit(),
                member,
            },
        );
    }
    Ok(member)
}

#[allow(clippy::too_many_arguments)]
fn require_primary_atom<D>(
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    definition_id: ObjectDefinitionPlanId,
    atom_id: ObjectDefinitionAtomId,
    role: InitializationArtifactRoleV1,
    expected_section: BuiltinObjectSectionRoleV1,
    expected_size: u64,
) -> Result<(u32, u64, u64), StrongInitializationRegistrationValidationError> {
    let definition = member.definitions().definition(definition_id).ok_or(
        StrongInitializationRegistrationValidationError::MissingVerifiedDefinition {
            unit: plan.semantic().unit(),
            definition: definition_id,
        },
    )?;
    if definition.primary_atom() != atom_id {
        return Err(
            StrongInitializationRegistrationValidationError::PrimaryAtomMismatch {
                unit: plan.semantic().unit(),
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
            StrongInitializationRegistrationValidationError::MissingPrimaryAtom {
                unit: plan.semantic().unit(),
                role,
                atom: atom_id,
            },
        )?;
    let (section, start, end) = atom_file_range(member, atom).map_err(|kind| {
        StrongInitializationRegistrationValidationError::InvalidPrimaryAtomFileRange {
            unit: plan.semantic().unit(),
            role,
            atom: atom_id,
            kind,
        }
    })?;
    if section != expected_section {
        return Err(
            StrongInitializationRegistrationValidationError::PrimaryAtomSectionMismatch {
                unit: plan.semantic().unit(),
                role,
            },
        );
    }
    if end - start != expected_size {
        return Err(
            StrongInitializationRegistrationValidationError::PrimaryAtomSizeMismatch {
                unit: plan.semantic().unit(),
                role,
                expected: expected_size,
                actual: end - start,
            },
        );
    }
    if atom.start() % 8 != 0 {
        return Err(
            StrongInitializationRegistrationValidationError::PrimaryAtomAlignmentMismatch {
                unit: plan.semantic().unit(),
                role,
                address: atom.start(),
            },
        );
    }
    if member
        .definitions()
        .strong_symbol_by_table_index(definition.primary_symbol_table_index())
        .is_none()
    {
        return Err(
            StrongInitializationRegistrationValidationError::MissingPrimarySymbol {
                unit: plan.semantic().unit(),
                role,
            },
        );
    }
    Ok((definition.primary_symbol_table_index(), start, end))
}

#[allow(clippy::too_many_arguments)]
fn require_patch<D>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    intent: DigestPatchIntentId,
    source: DigestNodeId,
    field: DigestSemanticFieldRole,
    member: SlibMemberId,
    file_start: u64,
    offset_within_atom: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, StrongInitializationRegistrationValidationError> {
    let patch = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == intent)
        .copied()
        .ok_or(
            StrongInitializationRegistrationValidationError::MissingPatch {
                unit: plan.semantic().unit(),
                intent,
            },
        )?;
    let expected_checked_offset = file_start.checked_add(offset_within_atom).ok_or(
        StrongInitializationRegistrationValidationError::RecordRangeOverflow(
            plan.semantic().unit(),
        ),
    )?;
    use InitializationRegistrationPatchFailureV1 as Failure;
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
    if let Some(failure) = failure {
        return Err(
            StrongInitializationRegistrationValidationError::PatchMismatch {
                unit: plan.semantic().unit(),
                intent,
                kind: failure,
            },
        );
    }
    Ok(patch)
}

fn validate_exact_atom_patch_set<D>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    member: SlibMemberId,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    let mut expected = vec![plan.registration_definition_patch()];
    if let Some(gateway) = plan.schedule().gateway_definition_patch() {
        expected.push(gateway);
    }
    if let Some(site) = patch_sites.sites().iter().find(|site| {
        site.member() == member
            && site.atom() == plan.registration_primary_atom()
            && !expected.contains(&site.intent())
    }) {
        return Err(
            StrongInitializationRegistrationValidationError::UnexpectedPatchInPrimaryAtom {
                unit: plan.semantic().unit(),
                intent: site.intent(),
            },
        );
    }
    Ok(())
}
