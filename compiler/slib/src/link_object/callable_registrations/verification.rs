use std::collections::BTreeMap;

use super::physical::{atom_file_range, validate_objects, verified_member};
use super::record::{DESCRIPTOR_SIZE, validate_record_bytes};
use super::{
    CallableRegistrationDigestPlanFailureV1, CallableRegistrationPatchFailureV1,
    CallableRegistrationRelocationFailureV1, StrongCallableRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    ScoopLirObjectCandidateV1, StrongRelocationBindingV1, StrongRelocationResolutionV1,
    VerifiedMaterializedPatchSiteV1, VerifiedObjectRelocationFormV1,
    VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_identity::{
    DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentId, DigestSemanticFieldRole,
    PersistentCallableBodyId, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    DigestFinalizationPlanV1, DigestInputRefV1, StrongCallableRegistrationPlanSetV1,
    StrongCallableRegistrationPlanV1,
};

mod context_keys;

const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const BODY_DEFINITION_FINGERPRINT_OFFSET: u64 = 152;
const ENTRY_POINTER_OFFSET: u64 = 184;
const DIGEST_WIDTH: u8 = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableRegistrationV1 {
    body: PersistentCallableBodyId,
    member: SlibMemberId,
    primary_symbol_table_index: u32,
    checked_offset: u64,
    entry_relocation: StrongRelocationBindingV1,
    registration_definition_patch: VerifiedMaterializedPatchSiteV1,
    body_definition_patch: VerifiedMaterializedPatchSiteV1,
}

impl VerifiedStrongCallableRegistrationV1 {
    pub const fn body(&self) -> PersistentCallableBodyId {
        self.body
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

    pub const fn entry_relocation(&self) -> &StrongRelocationBindingV1 {
        &self.entry_relocation
    }

    pub const fn registration_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.registration_definition_patch
    }

    pub const fn body_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.body_definition_patch
    }
}

/// Proof that every LIR callable has exactly one canonical provisional
/// registration record whose entry relocates to that callable's body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongCallableRegistrationSetV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongCallableRegistrationPlanSetV1,
    registrations: Vec<VerifiedStrongCallableRegistrationV1>,
}

impl VerifiedStrongCallableRegistrationSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }

    pub const fn plan(&self) -> &StrongCallableRegistrationPlanSetV1 {
        &self.plan
    }

    pub fn registrations(&self) -> &[VerifiedStrongCallableRegistrationV1] {
        &self.registrations
    }
}

pub fn verify_strong_callable_registrations_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongCallableRegistrationPlanSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongCallableRegistrationSetV1, StrongCallableRegistrationValidationError> {
    if patch_sites.producer() != plan.producer() {
        return Err(StrongCallableRegistrationValidationError::DigestPatchProducerMismatch);
    }

    let objects = validate_objects(patch_sites.builtins(), scoop_objects)?;
    let mut registrations = Vec::with_capacity(plan.registrations().len());
    for registration in plan.registrations() {
        validate_digest_graph(patch_sites.digest_plan(), *registration)?;
        let support = plan
            .runtime_scans()
            .callable(registration.body())
            .expect("the production plan contains support for every callable");
        registrations.push(verify_registration(
            &patch_sites,
            &objects,
            *registration,
            support.context_keys(),
        )?);
    }

    Ok(VerifiedStrongCallableRegistrationSetV1 {
        patch_sites,
        plan,
        registrations,
    })
}

fn validate_digest_graph(
    digest_plan: &DigestFinalizationPlanV1,
    plan: StrongCallableRegistrationPlanV1,
) -> Result<(), StrongCallableRegistrationValidationError> {
    use CallableRegistrationDigestPlanFailureV1 as Failure;

    let registration_object = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::object_definition(plan.primary_atom()))
        .ok_or(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::MissingRegistrationObjectDefinitionNode,
            },
        )?;
    if registration_object.id() != plan.registration_object_node() {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::RegistrationObjectDefinitionNodeIdentity,
            },
        );
    }
    let odr = matches!(
        plan.definition_owner(),
        scoop_lir::RegistrationDefinitionOwner::Odr { .. }
    );
    let expected_object_inputs = if odr {
        vec![DigestInputRefV1::ObjectDefinition(
            plan.body_definition_node(),
        )]
    } else {
        Vec::new()
    };
    if registration_object.direct_inputs() != expected_object_inputs {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::RegistrationObjectDefinitionDirectInputs,
            },
        );
    }
    if !registration_object.patch_intents().is_empty() {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::RegistrationObjectDefinitionPatchSet,
            },
        );
    }

    let body_definition = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &DigestNodeKey::object_definition(plan.body_primary_atom()))
        .ok_or(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::MissingBodyObjectDefinitionNode,
            },
        )?;
    if body_definition.id() != plan.body_definition_node() {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::BodyObjectDefinitionNodeIdentity,
            },
        );
    }

    let registration = digest_plan
        .nodes()
        .iter()
        .find(|node| node.key() == &plan.definition_owner().digest_key(plan.definition_plan()))
        .ok_or(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::MissingRegistrationNode,
            },
        )?;
    if registration.id() != plan.registration_fingerprint_node() {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::RegistrationNodeIdentity,
            },
        );
    }
    let mut expected_inputs = vec![DigestInputRefV1::from_node(registration_object)];
    if odr {
        let lir = digest_plan
            .nodes()
            .iter()
            .find(|node| node.key() == &DigestNodeKey::lir_definition(plan.primary_atom()))
            .ok_or(
                StrongCallableRegistrationValidationError::DigestPlanMismatch {
                    body: plan.body(),
                    kind: Failure::RegistrationDirectInputs,
                },
            )?;
        expected_inputs.push(DigestInputRefV1::from_node(lir));
    } else {
        expected_inputs.push(DigestInputRefV1::from_node(body_definition));
    }
    expected_inputs.sort_unstable();
    if registration.direct_inputs() != expected_inputs {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::RegistrationDirectInputs,
            },
        );
    }
    if registration.patch_intents().len() != 1
        || registration.patch_intents()[0].id() != plan.registration_definition_patch()
    {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::RegistrationPatchSet,
            },
        );
    }
    if !body_definition
        .patch_intents()
        .iter()
        .any(|patch| patch.id() == plan.body_definition_patch())
    {
        return Err(
            StrongCallableRegistrationValidationError::DigestPlanMismatch {
                body: plan.body(),
                kind: Failure::BodyDefinitionPatchMissing,
            },
        );
    }
    Ok(())
}

fn verify_registration(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: StrongCallableRegistrationPlanV1,
    keys: &[scoop_lir::CallableContextKeyCellV1],
) -> Result<VerifiedStrongCallableRegistrationV1, StrongCallableRegistrationValidationError> {
    let builtins = patch_sites.builtins();
    let member = required_scoop_member(builtins, plan, plan.definition_plan())?;
    let verified_member = verified_member(builtins, member)?;
    let definition = verified_member
        .definitions()
        .definition(plan.definition_plan())
        .ok_or(
            StrongCallableRegistrationValidationError::MissingVerifiedDefinition {
                body: plan.body(),
                definition: plan.definition_plan(),
            },
        )?;
    if definition.primary_atom() != plan.primary_atom() {
        return Err(
            StrongCallableRegistrationValidationError::PrimaryAtomMismatch {
                body: plan.body(),
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
            StrongCallableRegistrationValidationError::MissingPrimaryAtom {
                body: plan.body(),
                atom: plan.primary_atom(),
            },
        )?;
    let (section_role, file_start, file_end) =
        atom_file_range(verified_member, atom).map_err(|kind| {
            StrongCallableRegistrationValidationError::InvalidPrimaryAtomFileRange {
                body: plan.body(),
                atom: plan.primary_atom(),
                kind,
            }
        })?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(
            StrongCallableRegistrationValidationError::PrimaryAtomSectionMismatch {
                body: plan.body(),
            },
        );
    }
    let size = file_end - file_start;
    if size != DESCRIPTOR_SIZE as u64 {
        return Err(
            StrongCallableRegistrationValidationError::PrimaryAtomSizeMismatch {
                body: plan.body(),
                actual: size,
            },
        );
    }

    let entry_relocation = verify_entry_relocation(patch_sites, verified_member, plan)?;
    context_keys::verify(objects[&member], verified_member, plan, keys)?;
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
    let body_definition_patch = require_patch(
        patch_sites,
        plan,
        plan.body_definition_patch(),
        plan.body_definition_node(),
        DigestSemanticFieldRole::CallableBodyDefinition,
        member,
        file_start,
        BODY_DEFINITION_FINGERPRINT_OFFSET,
    )?;
    validate_exact_atom_patch_set(patch_sites, plan, member)?;
    validate_record_bytes(objects[&member], file_start, plan)?;

    Ok(VerifiedStrongCallableRegistrationV1 {
        body: plan.body(),
        member,
        primary_symbol_table_index: definition.primary_symbol_table_index(),
        checked_offset: file_start,
        entry_relocation,
        registration_definition_patch,
        body_definition_patch,
    })
}

fn required_scoop_member(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongCallableRegistrationPlanV1,
    definition: scoop_identity::ObjectDefinitionPlanId,
) -> Result<SlibMemberId, StrongCallableRegistrationValidationError> {
    let member = builtins
        .member_plan()
        .member_for_definition(definition)
        .ok_or(
            StrongCallableRegistrationValidationError::MissingDefinitionAssignment {
                body: plan.body(),
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
            StrongCallableRegistrationValidationError::DefinitionAssignedToNonScoopMember {
                body: plan.body(),
                member,
            },
        );
    }
    Ok(member)
}

fn verify_entry_relocation(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    registration_member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: StrongCallableRegistrationPlanV1,
) -> Result<StrongRelocationBindingV1, StrongCallableRegistrationValidationError> {
    use CallableRegistrationRelocationFailureV1 as Failure;

    let physical = registration_member
        .relocations()
        .iter()
        .filter(|relocation| {
            relocation.containing_atom() == plan.primary_atom()
                && relocation.offset_within_atom() == ENTRY_POINTER_OFFSET
        })
        .collect::<Vec<_>>();
    if physical.len() != 1 {
        return relocation_error(plan.body(), Failure::Count);
    }
    let bindings = patch_sites
        .builtins()
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == registration_member.member()
                && binding.containing_atom() == plan.primary_atom()
                && binding.offset_within_atom() == ENTRY_POINTER_OFFSET
        })
        .collect::<Vec<_>>();
    if bindings.len() != 1 {
        return relocation_error(plan.body(), Failure::Count);
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
    } else if binding.offset_within_atom() != ENTRY_POINTER_OFFSET {
        Some(Failure::OffsetWithinAtom)
    } else if binding.width_bytes() != 8 {
        Some(Failure::Width)
    } else if binding.relocation_form() != VerifiedObjectRelocationFormV1::Unsigned64 {
        Some(Failure::Form)
    } else if binding.encoded_value() != 0 {
        Some(Failure::EncodedValue)
    } else if binding.target_slot() != RelocationTargetSlotV1::Single {
        Some(Failure::TargetSlot)
    } else {
        None
    };
    if let Some(kind) = kind {
        return relocation_error(plan.body(), kind);
    }

    let body_member =
        required_scoop_member(patch_sites.builtins(), plan, plan.body_definition_plan())?;
    let body_verified_member = verified_member(patch_sites.builtins(), body_member)?;
    let body_definition = body_verified_member
        .definitions()
        .definition(plan.body_definition_plan())
        .ok_or(
            StrongCallableRegistrationValidationError::MissingVerifiedBodyDefinition {
                body: plan.body(),
                definition: plan.body_definition_plan(),
            },
        )?;
    if body_definition.primary_atom() != plan.body_primary_atom() {
        return Err(
            StrongCallableRegistrationValidationError::BodyPrimaryAtomMismatch {
                body: plan.body(),
                expected: plan.body_primary_atom(),
                actual: body_definition.primary_atom(),
            },
        );
    }
    let body_symbol = body_verified_member
        .definitions()
        .strong_symbol_by_table_index(body_definition.primary_symbol_table_index())
        .ok_or(
            StrongCallableRegistrationValidationError::MissingBodyPrimarySymbol {
                body: plan.body(),
            },
        )?;
    let expected_owner = LinkDefinitionOwnerV1::from_definition(
        body_symbol.definition_owner(),
        StrongDefinitionEntity::callable_body(plan.body()),
        StrongDefinitionRole::CallableBody,
    )
    .expect("callable body has a valid definition owner");
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
            return relocation_error(plan.body(), Failure::TargetDefinition);
        }
    };
    let kind = if target_definition != plan.body_definition_plan() {
        Some(Failure::TargetDefinition)
    } else if target_member != body_member {
        Some(Failure::TargetMember)
    } else if target_owner != expected_owner {
        Some(Failure::TargetOwner)
    } else if binding.symbol() != body_symbol.macho_name() {
        Some(Failure::TargetSymbol)
    } else {
        None
    };
    if let Some(kind) = kind {
        return relocation_error(plan.body(), kind);
    }
    Ok(binding.clone())
}

fn relocation_error<T>(
    body: PersistentCallableBodyId,
    kind: CallableRegistrationRelocationFailureV1,
) -> Result<T, StrongCallableRegistrationValidationError> {
    Err(StrongCallableRegistrationValidationError::EntryRelocationMismatch { body, kind })
}

#[allow(clippy::too_many_arguments)]
fn require_patch(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongCallableRegistrationPlanV1,
    intent: DigestPatchIntentId,
    source: DigestNodeId,
    field_role: DigestSemanticFieldRole,
    member: SlibMemberId,
    file_start: u64,
    offset_within_atom: u64,
) -> Result<VerifiedMaterializedPatchSiteV1, StrongCallableRegistrationValidationError> {
    let patch = patch_sites
        .sites()
        .iter()
        .find(|site| site.intent() == intent)
        .copied()
        .ok_or(StrongCallableRegistrationValidationError::MissingPatch {
            body: plan.body(),
            intent,
        })?;
    let expected_checked_offset = file_start
        .checked_add(offset_within_atom)
        .ok_or(StrongCallableRegistrationValidationError::RecordRangeOverflow(plan.body()))?;
    let kind = if patch.source() != source {
        Some(CallableRegistrationPatchFailureV1::Source)
    } else if patch.semantic_field_role() != field_role {
        Some(CallableRegistrationPatchFailureV1::SemanticFieldRole)
    } else if patch.member() != member {
        Some(CallableRegistrationPatchFailureV1::Member)
    } else if patch.definition() != plan.definition_plan() {
        Some(CallableRegistrationPatchFailureV1::Definition)
    } else if patch.atom() != plan.primary_atom() {
        Some(CallableRegistrationPatchFailureV1::Atom)
    } else if patch.atom_role() != DefinitionAtomRole::Primary {
        Some(CallableRegistrationPatchFailureV1::AtomRole)
    } else if patch.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(CallableRegistrationPatchFailureV1::SectionRole)
    } else if patch.offset_within_atom() != offset_within_atom {
        Some(CallableRegistrationPatchFailureV1::OffsetWithinAtom)
    } else if patch.checked_offset() != expected_checked_offset {
        Some(CallableRegistrationPatchFailureV1::CheckedOffset)
    } else if patch.width_bytes() != DIGEST_WIDTH {
        Some(CallableRegistrationPatchFailureV1::Width)
    } else {
        None
    };
    if let Some(kind) = kind {
        return Err(StrongCallableRegistrationValidationError::PatchMismatch {
            body: plan.body(),
            intent,
            kind,
        });
    }
    Ok(patch)
}

fn validate_exact_atom_patch_set(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongCallableRegistrationPlanV1,
    member: SlibMemberId,
) -> Result<(), StrongCallableRegistrationValidationError> {
    let expected = [
        plan.registration_definition_patch(),
        plan.body_definition_patch(),
    ];
    if let Some(site) = patch_sites.sites().iter().find(|site| {
        site.member() == member
            && site.atom() == plan.primary_atom()
            && !expected.contains(&site.intent())
    }) {
        return Err(
            StrongCallableRegistrationValidationError::UnexpectedPatchInPrimaryAtom {
                body: plan.body(),
                intent: site.intent(),
            },
        );
    }
    Ok(())
}
