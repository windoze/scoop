use std::collections::BTreeMap;

use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolRequest,
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
    StrongRelocationBindingV1, StrongRelocationResolutionV1, VerifiedDarwinArm64RelocationFormV1,
    VerifiedDarwinArm64RelocationShapeV1, VerifiedMemberObjectRelocationIndexV1,
    VerifiedRelocationTargetV1, VerifiedRelocationUseV1, VerifiedScoopLirDigestPatchSiteSetV1,
};

const COORDINATOR_DIAGNOSTIC_OFFSET: u64 = 40;
const COORDINATOR_CELL_OFFSET: u64 = 48;
const COORDINATOR_STORAGE_OFFSET: u64 = 56;
const COORDINATOR_FAILURE_OFFSET: u64 = 64;
const COORDINATOR_INITIALIZER_OFFSET: u64 = 72;
const COORDINATOR_ENSURE_OFFSET: u64 = 80;
const REGISTRATION_DIAGNOSTIC_OFFSET: u64 = 160;
const REGISTRATION_CELL_OFFSET: u64 = 176;
const REGISTRATION_STORAGE_OFFSET: u64 = 184;
const REGISTRATION_FAILURE_OFFSET: u64 = 192;
const REGISTRATION_INITIALIZER_OFFSET: u64 = 264;
const REGISTRATION_ENSURE_OFFSET: u64 = 272;
const REGISTRATION_GATEWAY_OFFSET: u64 = 344;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiagnosticTargetV1 {
    member: SlibMemberId,
    atom: ObjectDefinitionAtomId,
    section_ordinal: u8,
    value: u64,
}

pub(super) struct VerifiedInitializationRelocationsV1 {
    pub(super) coordinator_diagnostic: VerifiedRelocationUseV1,
    pub(super) coordinator_cell: StrongRelocationBindingV1,
    pub(super) coordinator_storage: StrongRelocationBindingV1,
    pub(super) coordinator_failure: StrongRelocationBindingV1,
    pub(super) coordinator_initializer: StrongRelocationBindingV1,
    pub(super) coordinator_ensure: StrongRelocationBindingV1,
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
    descriptor_member: &VerifiedMemberObjectRelocationIndexV1,
    registration_member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<VerifiedInitializationRelocationsV1, StrongInitializationRegistrationValidationError> {
    require_relocation_count(
        descriptor_member,
        plan.descriptor_primary_atom(),
        6,
        plan,
        InitializationRelocationRoleV1::CoordinatorDiagnostic,
    )?;
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

    let coordinator_diagnostic = require_physical_use(
        descriptor_member,
        plan.descriptor_primary_atom(),
        COORDINATOR_DIAGNOSTIC_OFFSET,
        plan,
        InitializationRelocationRoleV1::CoordinatorDiagnostic,
    )?;
    let registration_diagnostic = require_physical_use(
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_DIAGNOSTIC_OFFSET,
        plan,
        InitializationRelocationRoleV1::RegistrationDiagnostic,
    )?;
    let expected_diagnostic = expected_diagnostic_target(descriptor_member, plan)?;
    let coordinator_target = validate_diagnostic_target(
        objects,
        descriptor_member,
        &coordinator_diagnostic,
        expected_diagnostic,
        plan,
        InitializationRelocationRoleV1::CoordinatorDiagnostic,
    )?;
    let registration_target = validate_diagnostic_target(
        objects,
        registration_member,
        &registration_diagnostic,
        expected_diagnostic,
        plan,
        InitializationRelocationRoleV1::RegistrationDiagnostic,
    )?;
    if coordinator_target != registration_target {
        return relocation_error(
            plan,
            InitializationRelocationRoleV1::RegistrationDiagnostic,
            InitializationRelocationFailureV1::DiagnosticTarget,
        );
    }

    let cell_target = ExpectedStrongTargetV1 {
        definition: plan.cell_definition_plan(),
        atom: plan.cell_primary_atom(),
        entity: StrongDefinitionEntity::initialization_unit(plan.semantic().unit()),
        role: StrongDefinitionRole::InitializationCell,
        symbol: plan.cell_symbol(),
    };
    let coordinator_cell = verify_strong_relocation(
        patch_sites,
        descriptor_member,
        plan.descriptor_primary_atom(),
        COORDINATOR_CELL_OFFSET,
        cell_target,
        plan,
        InitializationRelocationRoleV1::CoordinatorCell,
    )?;
    let registration_cell = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_CELL_OFFSET,
        cell_target,
        plan,
        InitializationRelocationRoleV1::RegistrationCell,
    )?;

    let storage_target =
        static_storage_target(patch_sites, plan.definition_owner(), plan.storage(), false);
    let coordinator_storage = verify_strong_relocation(
        patch_sites,
        descriptor_member,
        plan.descriptor_primary_atom(),
        COORDINATOR_STORAGE_OFFSET,
        storage_target,
        plan,
        InitializationRelocationRoleV1::CoordinatorStorage,
    )?;
    let registration_storage = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_STORAGE_OFFSET,
        static_storage_target(patch_sites, plan.definition_owner(), plan.storage(), true),
        plan,
        InitializationRelocationRoleV1::RegistrationStorage,
    )?;

    let coordinator_failure = verify_strong_relocation(
        patch_sites,
        descriptor_member,
        plan.descriptor_primary_atom(),
        COORDINATOR_FAILURE_OFFSET,
        static_storage_target(
            patch_sites,
            plan.definition_owner(),
            plan.failure_root(),
            false,
        ),
        plan,
        InitializationRelocationRoleV1::CoordinatorFailureRoot,
    )?;
    let registration_failure = verify_strong_relocation(
        patch_sites,
        registration_member,
        plan.registration_primary_atom(),
        REGISTRATION_FAILURE_OFFSET,
        static_storage_target(
            patch_sites,
            plan.definition_owner(),
            plan.failure_root(),
            true,
        ),
        plan,
        InitializationRelocationRoleV1::RegistrationFailureRoot,
    )?;

    let coordinator_initializer = verify_strong_relocation(
        patch_sites,
        descriptor_member,
        plan.descriptor_primary_atom(),
        COORDINATOR_INITIALIZER_OFFSET,
        callable_target(plan.initializer()),
        plan,
        InitializationRelocationRoleV1::CoordinatorInitializer,
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
    let coordinator_ensure = verify_strong_relocation(
        patch_sites,
        descriptor_member,
        plan.descriptor_primary_atom(),
        COORDINATOR_ENSURE_OFFSET,
        callable_target(plan.ensure()),
        plan,
        InitializationRelocationRoleV1::CoordinatorEnsure,
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
        coordinator_diagnostic,
        coordinator_cell,
        coordinator_storage,
        coordinator_failure,
        coordinator_initializer,
        coordinator_ensure,
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
    } else if relocation.shape().form() != VerifiedDarwinArm64RelocationFormV1::Unsigned64 {
        Some(Failure::Form)
    } else if relocation.encoded_value() != 0 {
        Some(Failure::EncodedValue)
    } else {
        None
    };
    if let Some(failure) = failure {
        return relocation_error(plan, role, failure);
    }
    Ok(())
}

fn validate_diagnostic_target<D>(
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    member: &VerifiedMemberObjectRelocationIndexV1,
    relocation: &VerifiedRelocationUseV1,
    expected_target: DiagnosticTargetV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<DiagnosticTargetV1, StrongInitializationRegistrationValidationError> {
    let (section_ordinal, value) = match relocation.shape() {
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
            target:
                VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: Some(owner_atom),
                    section_ordinal,
                    value,
                    ..
                },
        } if *owner_atom == expected_target.atom => (*section_ordinal, *value),
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { .. } => {
            return relocation_error(plan, role, InitializationRelocationFailureV1::TargetKind);
        }
        _ => return relocation_error(plan, role, InitializationRelocationFailureV1::Form),
    };
    let section_index = usize::from(section_ordinal.get()) - 1;
    let sections = member.definitions().sections();
    if sections.roles().get(section_index) != Some(&BuiltinObjectSectionRoleV1::CString) {
        return relocation_error(
            plan,
            role,
            InitializationRelocationFailureV1::DiagnosticTarget,
        );
    }
    let section = sections
        .envelope()
        .sections()
        .get(section_index)
        .copied()
        .ok_or_else(|| {
            relocation_error_value(
                plan,
                role,
                InitializationRelocationFailureV1::DiagnosticTarget,
            )
        })?;
    let expected = plan
        .semantic()
        .diagnostic_path()
        .as_bytes()
        .iter()
        .copied()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target_end = value
        .checked_add(u64::try_from(expected.len()).unwrap())
        .ok_or_else(|| {
            relocation_error_value(
                plan,
                role,
                InitializationRelocationFailureV1::DiagnosticTarget,
            )
        })?;
    let section_end = section
        .virtual_address()
        .checked_add(section.byte_size())
        .expect("verified section range cannot overflow");
    if member.member() != expected_target.member
        || section_ordinal.get() != expected_target.section_ordinal
        || value != expected_target.value
        || value < section.virtual_address()
        || target_end > section_end
    {
        return relocation_error(
            plan,
            role,
            InitializationRelocationFailureV1::DiagnosticTarget,
        );
    }
    let file_start = section
        .file_offset()
        .and_then(|offset| {
            value
                .checked_sub(section.virtual_address())
                .and_then(|relative| offset.checked_add(relative))
        })
        .and_then(|offset| usize::try_from(offset).ok())
        .ok_or_else(|| {
            relocation_error_value(
                plan,
                role,
                InitializationRelocationFailureV1::DiagnosticTarget,
            )
        })?;
    let file_end = file_start.checked_add(expected.len()).ok_or_else(|| {
        relocation_error_value(
            plan,
            role,
            InitializationRelocationFailureV1::DiagnosticTarget,
        )
    })?;
    if objects
        .get(&member.member())
        .and_then(|object| object.get(file_start..file_end))
        != Some(expected.as_slice())
    {
        return relocation_error(
            plan,
            role,
            InitializationRelocationFailureV1::DiagnosticTarget,
        );
    }
    Ok(DiagnosticTargetV1 {
        member: member.member(),
        atom: expected_target.atom,
        section_ordinal: section_ordinal.get(),
        value,
    })
}

fn expected_diagnostic_target<D>(
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<DiagnosticTargetV1, StrongInitializationRegistrationValidationError> {
    let definition = member
        .definitions()
        .definition(plan.descriptor_definition_plan())
        .ok_or(
            StrongInitializationRegistrationValidationError::MissingVerifiedDefinition {
                unit: plan.semantic().unit(),
                definition: plan.descriptor_definition_plan(),
            },
        )?;
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| atom.atom() == plan.diagnostic_atom())
        .copied()
        .ok_or(
            StrongInitializationRegistrationValidationError::MissingDiagnosticAtom {
                unit: plan.semantic().unit(),
                atom: plan.diagnostic_atom(),
            },
        )?;
    let section_index = usize::from(atom.section_ordinal().get()) - 1;
    if member.definitions().sections().roles().get(section_index)
        != Some(&BuiltinObjectSectionRoleV1::CString)
    {
        return relocation_error(
            plan,
            InitializationRelocationRoleV1::CoordinatorDiagnostic,
            InitializationRelocationFailureV1::DiagnosticTarget,
        );
    }
    Ok(DiagnosticTargetV1 {
        member: member.member(),
        atom: atom.atom(),
        section_ordinal: atom.section_ordinal().get(),
        value: atom.start(),
    })
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
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    owner: scoop_lir::RegistrationDefinitionOwner,
    reference: StrongInitializationStaticStorageRefPlanV1,
    registration: bool,
) -> ExpectedStrongTargetV1 {
    if registration {
        ExpectedStrongTargetV1 {
            definition: reference.registration_definition_plan(),
            atom: reference.registration_primary_atom(),
            entity: StrongDefinitionEntity::static_storage(reference.storage()),
            role: StrongDefinitionRole::RootRegistration,
            symbol: reference.registration_symbol(),
        }
    } else {
        let definition = storage_definition(patch_sites.producer(), owner, reference.storage());
        ExpectedStrongTargetV1 {
            definition,
            atom: primary_atom(definition),
            entity: StrongDefinitionEntity::static_storage(reference.storage()),
            role: StrongDefinitionRole::StaticStorage,
            symbol: reference.storage_symbol(),
        }
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
    } else if binding.relocation_form() != VerifiedDarwinArm64RelocationFormV1::Unsigned64 {
        Some(Failure::Form)
    } else if binding.encoded_value() != 0 {
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
    let requested_symbol = scoop_lir::LirTargetProfile::DARWIN_AARCH64
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

fn storage_definition(
    producer: scoop_identity::ConeIdentity,
    owner: scoop_lir::RegistrationDefinitionOwner,
    storage: scoop_identity::PersistentStaticStorageId,
) -> ObjectDefinitionPlanId {
    let key = match owner {
        scoop_lir::RegistrationDefinitionOwner::Strong => ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::static_storage(storage),
            StrongDefinitionRole::StaticStorage,
        )
        .expect("static storage has a valid definition role"),
        scoop_lir::RegistrationDefinitionOwner::Odr { group, .. } => {
            let member = scoop_identity::OdrMemberKey::new(
                group,
                scoop_identity::OdrMemberRole::StaticStorage,
                scoop_identity::OdrMemberDiscriminator::StaticStorage(storage),
            )
            .expect("a delegated storage has a valid member role");
            ObjectDefinitionPlanKey::odr(
                scoop_identity::OdrMemberId::from_key(&member)
                    .expect("a delegated storage member is hashable"),
            )
        }
    };
    ObjectDefinitionPlanId::from_key(&key).expect("a static storage definition is hashable")
}

fn primary_atom(definition: ObjectDefinitionPlanId) -> ObjectDefinitionAtomId {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .expect("initialization relocation target primary atom is hashable")
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
