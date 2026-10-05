mod diagnostic;
use diagnostic::{expected_diagnostic_target, validate_diagnostic_target};

use std::collections::BTreeMap;

use scoop_identity::{
    DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    StrongInitializationCallableRefPlanV1, StrongInitializationRegistrationSchedulePlanV1,
    StrongInitializationStaticStorageRefPlanV1, StrongInitializationUnitRegistrationPlan,
};

use super::physical::verified_member;
use super::verification::required_scoop_member;
use super::{
    InitializationRelocationFailureV1, InitializationRelocationRoleV1,
    StrongInitializationRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    StrongRelocationBindingV1, StrongRelocationResolutionV1, VerifiedMemberObjectRelocationIndexV1,
    VerifiedRelocationUseV1, VerifiedScoopLirDigestPatchSiteSetV1,
};

const REGISTRATION_DIAGNOSTIC_OFFSET: u64 = 160;
const REGISTRATION_CELL_OFFSET: u64 = 176;
const REGISTRATION_STORAGE_OFFSET: u64 = 184;
const REGISTRATION_FAILURE_OFFSET: u64 = 192;
const REGISTRATION_INITIALIZER_OFFSET: u64 = 264;
const REGISTRATION_ENSURE_OFFSET: u64 = 272;
const REGISTRATION_GATEWAY_OFFSET: u64 = 344;

pub(super) struct VerifiedInitializationRelocationsV1 {
    pub(super) registration_diagnostic: VerifiedRelocationUseV1,
    pub(super) registration_cell: StrongRelocationBindingV1,
    pub(super) registration_storage: StrongRelocationBindingV1,
    pub(super) registration_failure: StrongRelocationBindingV1,
    pub(super) registration_initializer: StrongRelocationBindingV1,
    pub(super) registration_ensure: StrongRelocationBindingV1,
    pub(super) registration_gateway: Option<StrongRelocationBindingV1>,
}

pub(super) fn verify_relocations<D>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    registration_member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<VerifiedInitializationRelocationsV1, StrongInitializationRegistrationValidationError> {
    let expected_registration_count = if plan.schedule().gateway().is_some() {
        7
    } else {
        6
    };
    require_relocation_count(
        registration_member,
        plan.registration_primary_atom(),
        expected_registration_count,
        plan,
        InitializationRelocationRoleV1::RegistrationDiagnostic,
    )?;

    let registration_diagnostic = require_physical_use(
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_DIAGNOSTIC_OFFSET,
        plan,
        InitializationRelocationRoleV1::RegistrationDiagnostic,
    )?;
    let expected_diagnostic = expected_diagnostic_target(registration_member, plan)?;
    validate_diagnostic_target(
        objects,
        registration_member,
        &registration_diagnostic,
        expected_diagnostic,
        plan,
        InitializationRelocationRoleV1::RegistrationDiagnostic,
    )?;
    let cell_target = ExpectedStrongTargetV1 {
        definition: plan.cell_definition_plan(),
        atom: plan.cell_primary_atom(),
        entity: StrongDefinitionEntity::initialization_unit(plan.semantic().unit()),
        role: StrongDefinitionRole::InitializationCell,
        symbol: plan.cell_symbol(),
    };
    let registration_cell = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_CELL_OFFSET,
        cell_target,
        plan,
        InitializationRelocationRoleV1::RegistrationCell,
    )?;

    let registration_storage = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_STORAGE_OFFSET,
        static_storage_target(plan.storage()),
        plan,
        InitializationRelocationRoleV1::RegistrationStorage,
    )?;

    let registration_failure = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_FAILURE_OFFSET,
        static_storage_target(plan.failure_root()),
        plan,
        InitializationRelocationRoleV1::RegistrationFailureRoot,
    )?;

    let registration_initializer = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_INITIALIZER_OFFSET,
        callable_target(plan.initializer()),
        plan,
        InitializationRelocationRoleV1::RegistrationInitializer,
    )?;
    let registration_ensure = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_ENSURE_OFFSET,
        callable_target(plan.ensure()),
        plan,
        InitializationRelocationRoleV1::RegistrationEnsure,
    )?;
    let registration_gateway = match plan.schedule() {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup { gateway, .. } => {
            Some(verify_strong_relocation(
                patch_sites,
                registration_member,
                plan.registration_primary_atom(),
                REGISTRATION_GATEWAY_OFFSET,
                callable_target(**gateway),
                plan,
                InitializationRelocationRoleV1::RegistrationGateway,
            )?)
        }
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => None,
    };

    Ok(VerifiedInitializationRelocationsV1 {
        registration_diagnostic,
        registration_cell,
        registration_storage,
        registration_failure,
        registration_initializer,
        registration_ensure,
        registration_gateway,
    })
}

fn require_relocation_count<D>(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: ObjectDefinitionAtomId,
    expected: usize,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    if member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == atom)
        .count()
        != expected
    {
        return relocation_error(plan, role, InitializationRelocationFailureV1::Count);
    }
    Ok(())
}

fn require_physical_use<D>(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<VerifiedRelocationUseV1, StrongInitializationRegistrationValidationError> {
    let matches = member
        .relocations()
        .iter()
        .filter(|relocation| {
            relocation.containing_atom() == atom && relocation.offset_within_atom() == offset
        })
        .collect::<Vec<_>>();
    let [relocation] = matches.as_slice() else {
        return relocation_error(plan, role, InitializationRelocationFailureV1::MissingOffset);
    };
    validate_use_shape(relocation, atom, offset, plan, role)?;
    Ok((*relocation).clone())
}

fn validate_use_shape<D>(
    relocation: &VerifiedRelocationUseV1,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    use InitializationRelocationFailureV1 as Failure;
    let failure = if relocation.containing_atom() != atom {
        Some(Failure::ContainingAtom)
    } else if relocation.containing_atom_role() != DefinitionAtomRole::Primary {
        Some(Failure::ContainingAtomRole)
    } else if relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(Failure::SectionRole)
    } else if relocation.offset_within_atom() != offset {
        Some(Failure::MissingOffset)
    } else if relocation.width_bytes() != 8 {
        Some(Failure::Width)
    } else if !relocation.shape().form().is_absolute64() {
        Some(Failure::Form)
    } else {
        None
    };
    if let Some(failure) = failure {
        return relocation_error(plan, role, failure);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ExpectedStrongTargetV1 {
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
    symbol: PersistentSymbolRequest,
}

fn static_storage_target(
    reference: StrongInitializationStaticStorageRefPlanV1,
) -> ExpectedStrongTargetV1 {
    ExpectedStrongTargetV1 {
        definition: reference.registration_definition_plan(),
        atom: reference.registration_primary_atom(),
        entity: StrongDefinitionEntity::static_storage(reference.storage()),
        role: StrongDefinitionRole::RootRegistration,
        symbol: reference.registration_symbol(),
    }
}

fn callable_target(reference: StrongInitializationCallableRefPlanV1) -> ExpectedStrongTargetV1 {
    ExpectedStrongTargetV1 {
        definition: reference.body_definition_plan(),
        atom: reference.body_primary_atom(),
        entity: StrongDefinitionEntity::callable_body(reference.body()),
        role: StrongDefinitionRole::CallableBody,
        symbol: reference.entry_symbol(),
    }
}

fn verify_strong_relocation<D>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    source_member: &VerifiedMemberObjectRelocationIndexV1,
    source_atom: ObjectDefinitionAtomId,
    offset: u64,
    expected: ExpectedStrongTargetV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<StrongRelocationBindingV1, StrongInitializationRegistrationValidationError> {
    let _ = require_physical_use(source_member, source_atom, offset, plan, role)?;
    let matches = patch_sites
        .builtins()
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == source_member.member()
                && binding.containing_atom() == source_atom
                && binding.offset_within_atom() == offset
        })
        .collect::<Vec<_>>();
    let [binding] = matches.as_slice() else {
        return relocation_error(plan, role, InitializationRelocationFailureV1::Count);
    };
    validate_binding_shape(binding, source_atom, offset, plan, role)?;
    validate_strong_target(patch_sites, binding, expected, plan, role)?;
    Ok((*binding).clone())
}

fn validate_binding_shape<D>(
    binding: &StrongRelocationBindingV1,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    use InitializationRelocationFailureV1 as Failure;
    let failure = if binding.containing_atom() != atom {
        Some(Failure::ContainingAtom)
    } else if binding.containing_atom_role() != DefinitionAtomRole::Primary {
        Some(Failure::ContainingAtomRole)
    } else if binding.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(Failure::SectionRole)
    } else if binding.offset_within_atom() != offset {
        Some(Failure::MissingOffset)
    } else if binding.width_bytes() != 8 {
        Some(Failure::Width)
    } else if !binding.relocation_form().is_absolute64() {
        Some(Failure::Form)
    } else if binding
        .relocation_form()
        .absolute64_addend(binding.encoded_value())
        != Some(0)
    {
        Some(Failure::EncodedValue)
    } else if binding.target_slot() != RelocationTargetSlotV1::Single {
        Some(Failure::TargetSlot)
    } else {
        None
    };
    if let Some(failure) = failure {
        return relocation_error(plan, role, failure);
    }
    Ok(())
}

fn validate_strong_target<D>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    binding: &StrongRelocationBindingV1,
    expected: ExpectedStrongTargetV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    use InitializationRelocationFailureV1 as Failure;
    let target_member = required_scoop_member(patch_sites.builtins(), plan, expected.definition)?;
    let target = verified_member(patch_sites.builtins(), target_member)?;
    let definition = target.definitions().definition(expected.definition).ok_or(
        StrongInitializationRegistrationValidationError::MissingVerifiedDefinition {
            unit: plan.semantic().unit(),
            definition: expected.definition,
        },
    )?;
    if definition.primary_atom() != expected.atom {
        return relocation_error(plan, role, Failure::TargetDefinition);
    }
    let symbol = target
        .definitions()
        .strong_symbol_by_table_index(definition.primary_symbol_table_index())
        .ok_or_else(|| relocation_error_value(plan, role, Failure::TargetSymbol))?;
    let expected_owner = LinkDefinitionOwnerV1::from_definition(
        symbol.definition_owner(),
        expected.entity,
        expected.role,
    )
    .expect("the verified initialization target has a complete definition owner");
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
            return relocation_error(plan, role, Failure::TargetKind);
        }
    };
    let requested_symbol = patch_sites
        .builtins()
        .member_plan()
        .target()
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(expected.symbol.symbol().as_str())
        .into_bytes();
    let failure = if actual_definition != expected.definition {
        Some(Failure::TargetDefinition)
    } else if actual_member != target_member {
        Some(Failure::TargetMember)
    } else if actual_owner != expected_owner {
        Some(Failure::TargetOwner)
    } else if binding.symbol() != symbol.macho_name() || binding.symbol() != requested_symbol {
        Some(Failure::TargetSymbol)
    } else {
        None
    };
    if let Some(failure) = failure {
        return relocation_error(plan, role, failure);
    }
    Ok(())
}

fn relocation_error<T, D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
    kind: InitializationRelocationFailureV1,
) -> Result<T, StrongInitializationRegistrationValidationError> {
    Err(relocation_error_value(plan, role, kind))
}

fn relocation_error_value<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
    kind: InitializationRelocationFailureV1,
) -> StrongInitializationRegistrationValidationError {
    StrongInitializationRegistrationValidationError::RelocationMismatch {
        unit: plan.semantic().unit(),
        role,
        kind,
    }
}
