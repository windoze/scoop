use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    StaticStorageRelocationTableArtifactV1, StrongStaticStorageInitialArtifactPlanV1,
    StrongStaticStorageRegistrationPlanV1,
};

use super::physical::verified_member;
use super::verification::required_scoop_member;
use super::{
    StaticStorageRegistrationRelocationFailureV1, StaticStorageRelocationRoleV1,
    StrongStaticStorageRegistrationValidationError,
};
use crate::link_object::{
    BuiltinObjectSectionRoleV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    StrongRelocationBindingV1, StrongRelocationResolutionV1, VerifiedDarwinArm64RelocationFormV1,
    VerifiedDarwinArm64RelocationShapeV1, VerifiedMemberObjectRelocationIndexV1,
    VerifiedRelocationTargetV1, VerifiedRelocationUseV1, VerifiedScoopLirDigestPatchSiteSetV1,
};

const STORAGE_POINTER_OFFSET: u64 = 160;
const SCAN_POINTER_OFFSET: u64 = 192;
const TEMPLATE_POINTER_OFFSET: u64 = 264;
const RELOCATION_TABLE_POINTER_OFFSET: u64 = 280;

pub(super) struct VerifiedStaticStorageRelocations {
    pub(super) storage: StrongRelocationBindingV1,
    pub(super) scan: StrongRelocationBindingV1,
    pub(super) template: VerifiedRelocationUseV1,
    pub(super) relocation_table: VerifiedRelocationUseV1,
    pub(super) initial_storage: Vec<StrongRelocationBindingV1>,
    pub(super) initial_targets: Vec<StrongRelocationBindingV1>,
}

pub(super) fn verify_relocations(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    registration_member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> Result<VerifiedStaticStorageRelocations, StrongStaticStorageRegistrationValidationError> {
    let descriptor_relocations = registration_member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == plan.registration_primary_atom())
        .collect::<Vec<_>>();
    if descriptor_relocations.len() != 4 {
        return relocation_error(
            plan,
            StaticStorageRelocationRoleV1::StoragePointer,
            StaticStorageRegistrationRelocationFailureV1::Count,
        );
    }
    let storage_use = descriptor_use(
        &descriptor_relocations,
        plan,
        STORAGE_POINTER_OFFSET,
        StaticStorageRelocationRoleV1::StoragePointer,
    )?;
    let scan_use = descriptor_use(
        &descriptor_relocations,
        plan,
        SCAN_POINTER_OFFSET,
        StaticStorageRelocationRoleV1::ScanProgramPointer,
    )?;
    let template = descriptor_use(
        &descriptor_relocations,
        plan,
        TEMPLATE_POINTER_OFFSET,
        StaticStorageRelocationRoleV1::InitialTemplatePointer,
    )?;
    let relocation_table = descriptor_use(
        &descriptor_relocations,
        plan,
        RELOCATION_TABLE_POINTER_OFFSET,
        StaticStorageRelocationRoleV1::InitialRelocationTablePointer,
    )?;

    let storage = strong_binding_at(
        patch_sites,
        registration_member.member(),
        plan.registration_primary_atom(),
        STORAGE_POINTER_OFFSET,
        plan,
        StaticStorageRelocationRoleV1::StoragePointer,
    )?;
    validate_strong_target(
        patch_sites,
        plan,
        &storage,
        plan.storage_definition_plan(),
        plan.storage_primary_atom(),
        StrongDefinitionEntity::static_storage(plan.semantic().storage()),
        StrongDefinitionRole::StaticStorage,
        plan.semantic().symbol(),
        StaticStorageRelocationRoleV1::StoragePointer,
    )?;
    let scan = strong_binding_at(
        patch_sites,
        registration_member.member(),
        plan.registration_primary_atom(),
        SCAN_POINTER_OFFSET,
        plan,
        StaticStorageRelocationRoleV1::ScanProgramPointer,
    )?;
    if matches!(
        plan.semantic().value_layout(),
        scoop_lir::StaticStorageLayout::External(_)
    ) {
        let expected = scoop_lir::LirTargetProfile::DARWIN_AARCH64
            .contract()
            .native_symbol_normalization()
            .compiler_generated_object_symbol(plan.scan_symbol().symbol().as_str())
            .into_bytes();
        if !matches!(
            scan.resolution(),
            StrongRelocationResolutionV1::ExternalCandidate { .. }
        ) || scan.symbol() != expected
        {
            return relocation_error(
                plan,
                StaticStorageRelocationRoleV1::ScanProgramPointer,
                StaticStorageRegistrationRelocationFailureV1::TargetSymbol,
            );
        }
        // The shared shape-link import resolves this actual provider definition.
    } else {
        validate_strong_target(
            patch_sites,
            plan,
            &scan,
            plan.scan_definition_plan(),
            plan.scan_primary_atom(),
            StrongDefinitionEntity::scan(plan.semantic().scan()),
            StrongDefinitionRole::ScanProgram,
            plan.scan_symbol(),
            StaticStorageRelocationRoleV1::ScanProgramPointer,
        )?;
    }
    let _ = (storage_use, scan_use);

    match plan.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {
            validate_sentinel_target(
                template,
                registration_member,
                plan,
                StaticStorageRelocationRoleV1::InitialTemplatePointer,
            )?;
            validate_sentinel_target(
                relocation_table,
                registration_member,
                plan,
                StaticStorageRelocationRoleV1::InitialRelocationTablePointer,
            )?;
        }
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            template_atom,
            relocation_table: table,
        } => {
            validate_local_atom_target(
                template,
                template_atom,
                registration_member,
                plan,
                StaticStorageRelocationRoleV1::InitialTemplatePointer,
            )?;
            match table {
                StaticStorageRelocationTableArtifactV1::SharedEmptySentinel => {
                    validate_sentinel_target(
                        relocation_table,
                        registration_member,
                        plan,
                        StaticStorageRelocationRoleV1::InitialRelocationTablePointer,
                    )?;
                }
                StaticStorageRelocationTableArtifactV1::Defined { atom } => {
                    validate_local_atom_target(
                        relocation_table,
                        atom,
                        registration_member,
                        plan,
                        StaticStorageRelocationRoleV1::InitialRelocationTablePointer,
                    )?;
                }
            }
        }
    }

    let storage_member =
        required_scoop_member(patch_sites.builtins(), plan, plan.storage_definition_plan())?;
    let storage_index = verified_member(patch_sites.builtins(), storage_member)?;
    let storage_physical = storage_index
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == plan.storage_primary_atom())
        .count();
    let expected_initial_count = plan.semantic().initial_state().immortal_relocations().len();
    if storage_physical != expected_initial_count {
        return relocation_error(
            plan,
            StaticStorageRelocationRoleV1::InitialStorageValue { index: 0 },
            StaticStorageRegistrationRelocationFailureV1::Count,
        );
    }

    let table_atom = match plan.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: StaticStorageRelocationTableArtifactV1::Defined { atom },
            ..
        } => Some(atom),
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
        | StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: StaticStorageRelocationTableArtifactV1::SharedEmptySentinel,
            ..
        } => None,
    };
    let table_physical = table_atom.map_or(0, |atom| {
        storage_index
            .relocations()
            .iter()
            .filter(|relocation| relocation.containing_atom() == atom)
            .count()
    });
    if table_physical != expected_initial_count {
        return relocation_error(
            plan,
            StaticStorageRelocationRoleV1::InitialRelocationTarget { index: 0 },
            StaticStorageRegistrationRelocationFailureV1::Count,
        );
    }

    let mut initial_storage = Vec::with_capacity(expected_initial_count);
    let mut initial_targets = Vec::with_capacity(expected_initial_count);
    for (index, (relocation, registration_symbol)) in plan
        .semantic()
        .initial_state()
        .immortal_relocations()
        .iter()
        .zip(plan.immortal_registration_symbols())
        .enumerate()
    {
        let index = u32::try_from(index).expect("static relocation count fits u32");
        let object_role = StaticStorageRelocationRoleV1::InitialStorageValue { index };
        let object_definition = strong_definition(
            patch_sites.producer(),
            StrongDefinitionEntity::immortal_object(relocation.target()),
            StrongDefinitionRole::ImmortalObject,
        );
        let object_atom = primary_atom(object_definition);
        let object_symbol = PersistentSymbolRequest::new(
            scoop_identity::PersistentSymbolKey::ImmortalObject(relocation.target()),
            scoop_identity::LinkageClass::ConeStrong,
        )
        .expect("immortal object symbol is valid");
        let storage_binding = strong_binding_at(
            patch_sites,
            storage_member,
            plan.storage_primary_atom(),
            relocation.pointer_offset(),
            plan,
            object_role,
        )?;
        validate_strong_target(
            patch_sites,
            plan,
            &storage_binding,
            object_definition,
            object_atom,
            StrongDefinitionEntity::immortal_object(relocation.target()),
            StrongDefinitionRole::ImmortalObject,
            object_symbol,
            object_role,
        )?;
        initial_storage.push(storage_binding);

        let table_atom = table_atom.expect("nonempty relocation plan has a table atom");
        let target_role = StaticStorageRelocationRoleV1::InitialRelocationTarget { index };
        let registration_definition = strong_definition(
            patch_sites.producer(),
            StrongDefinitionEntity::immortal_object(relocation.target()),
            StrongDefinitionRole::ImmortalRegistration,
        );
        let table_binding = strong_binding_at(
            patch_sites,
            storage_member,
            table_atom,
            u64::from(index) * 16 + 8,
            plan,
            target_role,
        )?;
        validate_strong_target(
            patch_sites,
            plan,
            &table_binding,
            registration_definition,
            primary_atom(registration_definition),
            StrongDefinitionEntity::immortal_object(relocation.target()),
            StrongDefinitionRole::ImmortalRegistration,
            *registration_symbol,
            target_role,
        )?;
        initial_targets.push(table_binding);
    }

    Ok(VerifiedStaticStorageRelocations {
        storage,
        scan,
        template: template.clone(),
        relocation_table: relocation_table.clone(),
        initial_storage,
        initial_targets,
    })
}

fn descriptor_use<'a>(
    relocations: &[&'a VerifiedRelocationUseV1],
    plan: &StrongStaticStorageRegistrationPlanV1,
    offset: u64,
    role: StaticStorageRelocationRoleV1,
) -> Result<&'a VerifiedRelocationUseV1, StrongStaticStorageRegistrationValidationError> {
    let relocation = relocations
        .iter()
        .find(|relocation| relocation.offset_within_atom() == offset)
        .copied()
        .ok_or_else(|| {
            relocation_error_value(
                plan,
                role,
                StaticStorageRegistrationRelocationFailureV1::MissingOffset,
            )
        })?;
    validate_use_shape(
        relocation,
        plan.registration_primary_atom(),
        offset,
        plan,
        role,
    )?;
    Ok(relocation)
}

fn strong_binding_at(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    member: crate::SlibMemberId,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
) -> Result<StrongRelocationBindingV1, StrongStaticStorageRegistrationValidationError> {
    let bindings = patch_sites
        .builtins()
        .strong_relocations()
        .bindings()
        .iter()
        .filter(|binding| {
            binding.source_member() == member
                && binding.containing_atom() == atom
                && binding.offset_within_atom() == offset
        })
        .collect::<Vec<_>>();
    if bindings.len() != 1 {
        return relocation_error(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::Count,
        );
    }
    let binding = bindings[0];
    validate_binding_shape(binding, atom, offset, plan, role)?;
    Ok(binding.clone())
}

#[allow(clippy::too_many_arguments)]
fn validate_strong_target(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    binding: &StrongRelocationBindingV1,
    expected_definition: ObjectDefinitionPlanId,
    expected_primary_atom: ObjectDefinitionAtomId,
    expected_entity: StrongDefinitionEntity,
    expected_role: StrongDefinitionRole,
    expected_symbol: PersistentSymbolRequest,
    relocation_role: StaticStorageRelocationRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    use StaticStorageRegistrationRelocationFailureV1 as Failure;

    let target_member = required_scoop_member(patch_sites.builtins(), plan, expected_definition)?;
    let target = verified_member(patch_sites.builtins(), target_member)?;
    let definition = target.definitions().definition(expected_definition).ok_or(
        StrongStaticStorageRegistrationValidationError::MissingVerifiedDefinition {
            storage: plan.semantic().storage(),
            definition: expected_definition,
        },
    )?;
    if definition.primary_atom() != expected_primary_atom {
        return relocation_error(plan, relocation_role, Failure::TargetAtom);
    }
    let symbol = target
        .definitions()
        .strong_symbol_by_table_index(definition.primary_symbol_table_index())
        .ok_or_else(|| relocation_error_value(plan, relocation_role, Failure::TargetSymbol))?;
    let expected_owner = LinkDefinitionOwnerV1::from_strong_primary(expected_entity, expected_role)
        .expect("static-storage relocation target is a valid strong owner");
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
            return relocation_error(plan, relocation_role, Failure::TargetDefinition);
        }
    };
    let normalization = scoop_lir::LirTargetProfile::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization();
    let requested_symbol = normalization
        .compiler_generated_object_symbol(expected_symbol.symbol().as_str())
        .into_bytes();
    let failure = if actual_definition != expected_definition {
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
        return relocation_error(plan, relocation_role, failure);
    }
    Ok(())
}

fn validate_local_atom_target(
    relocation: &VerifiedRelocationUseV1,
    expected_atom: ObjectDefinitionAtomId,
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    if relocation.encoded_value() != 0 {
        return relocation_error(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::EncodedValue,
        );
    }
    match relocation.shape() {
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
            target:
                VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: Some(owner_atom),
                    section_ordinal,
                    ..
                },
        } => {
            if *owner_atom != expected_atom {
                return relocation_error(
                    plan,
                    role,
                    StaticStorageRegistrationRelocationFailureV1::TargetAtom,
                );
            }
            let section_index = usize::from(section_ordinal.get()) - 1;
            if member.definitions().sections().roles().get(section_index)
                != Some(&BuiltinObjectSectionRoleV1::ReadOnlyData)
            {
                return relocation_error(
                    plan,
                    role,
                    StaticStorageRegistrationRelocationFailureV1::TargetSection,
                );
            }
            Ok(())
        }
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { .. } => relocation_error(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::TargetKind,
        ),
        _ => relocation_error(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::Form,
        ),
    }
}

fn validate_sentinel_target(
    relocation: &VerifiedRelocationUseV1,
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let (section_index, target) = match relocation.shape() {
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
            target:
                VerifiedRelocationTargetV1::SectionBase {
                    section_ordinal,
                    section_role: BuiltinObjectSectionRoleV1::ReadOnlyData,
                },
        } => {
            let index = usize::try_from(section_ordinal.get()).unwrap() - 1;
            let section = member.definitions().sections().envelope().sections()[index];
            (index, section.virtual_address())
        }
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
            target:
                VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: None,
                    section_ordinal,
                    value,
                    ..
                },
        } => (usize::from(section_ordinal.get()) - 1, *value),
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { .. } => {
            return relocation_error(
                plan,
                role,
                StaticStorageRegistrationRelocationFailureV1::TargetKind,
            );
        }
        _ => {
            return relocation_error(
                plan,
                role,
                StaticStorageRegistrationRelocationFailureV1::Form,
            );
        }
    };
    let sections = member.definitions().sections();
    if sections.roles().get(section_index) != Some(&BuiltinObjectSectionRoleV1::ReadOnlyData) {
        return relocation_error(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::TargetSection,
        );
    }
    let section = sections.envelope().sections()[section_index];
    let target = target
        .checked_add(relocation.encoded_value())
        .ok_or_else(|| {
            relocation_error_value(
                plan,
                role,
                StaticStorageRegistrationRelocationFailureV1::EncodedValue,
            )
        })?;
    let (extent, alignment) = match role {
        StaticStorageRelocationRoleV1::InitialTemplatePointer => (1, 1),
        StaticStorageRelocationRoleV1::InitialRelocationTablePointer => (16, 8),
        _ => unreachable!("only initial-state pointers may target sentinels"),
    };
    let target_end = target.checked_add(extent).ok_or_else(|| {
        relocation_error_value(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::TargetSection,
        )
    })?;
    let section_end = section
        .virtual_address()
        .checked_add(section.byte_size())
        .expect("validated section range cannot overflow");
    if target < section.virtual_address()
        || target_end > section_end
        || target % alignment != 0
        || member
            .definitions()
            .definitions()
            .iter()
            .flat_map(|definition| definition.atoms())
            .any(|atom| {
                atom.section_ordinal().get() as usize == section_index + 1
                    && atom.start() < target_end
                    && target < atom.end()
            })
    {
        return relocation_error(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::TargetSection,
        );
    }
    Ok(())
}

fn validate_use_shape(
    relocation: &VerifiedRelocationUseV1,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    use StaticStorageRegistrationRelocationFailureV1 as Failure;
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
    } else {
        None
    };
    if let Some(failure) = failure {
        return relocation_error(plan, role, failure);
    }
    Ok(())
}

pub(super) fn sentinel_target_key(
    member: &VerifiedMemberObjectRelocationIndexV1,
    relocation: &VerifiedRelocationUseV1,
) -> Option<(crate::SlibMemberId, u32, u64)> {
    let (section, base) = match relocation.shape() {
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
            target:
                VerifiedRelocationTargetV1::SectionBase {
                    section_ordinal, ..
                },
        } => {
            let index = usize::try_from(section_ordinal.get())
                .ok()?
                .checked_sub(1)?;
            let section_record = member
                .definitions()
                .sections()
                .envelope()
                .sections()
                .get(index)?;
            (section_ordinal.get(), section_record.virtual_address())
        }
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
            target:
                VerifiedRelocationTargetV1::LocalDefinition {
                    owner_atom: None,
                    section_ordinal,
                    value,
                    ..
                },
        } => (u32::from(section_ordinal.get()), *value),
        _ => return None,
    };
    Some((
        member.member(),
        section,
        base.checked_add(relocation.encoded_value())?,
    ))
}

fn validate_binding_shape(
    binding: &StrongRelocationBindingV1,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    use StaticStorageRegistrationRelocationFailureV1 as Failure;
    let expected_section = match role {
        StaticStorageRelocationRoleV1::InitialStorageValue { .. } => {
            BuiltinObjectSectionRoleV1::WritableData
        }
        _ => BuiltinObjectSectionRoleV1::ReadOnlyData,
    };
    let expected_atom_role = match role {
        StaticStorageRelocationRoleV1::InitialRelocationTarget { .. } => {
            DefinitionAtomRole::RuntimeRecord
        }
        _ => DefinitionAtomRole::Primary,
    };
    let failure = if binding.containing_atom() != atom {
        Some(Failure::ContainingAtom)
    } else if binding.containing_atom_role() != expected_atom_role {
        Some(Failure::ContainingAtomRole)
    } else if binding.section_role() != expected_section {
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

fn strong_definition(
    producer: scoop_identity::ConeIdentity,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> ObjectDefinitionPlanId {
    let key = ObjectDefinitionPlanKey::strong(producer, entity, role)
        .expect("static relocation target has a valid entity/role pair");
    ObjectDefinitionPlanId::from_key(&key).expect("strong definition is hashable")
}

fn primary_atom(definition: ObjectDefinitionPlanId) -> ObjectDefinitionAtomId {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .expect("strong primary atom is hashable")
}

fn relocation_error<T>(
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
    kind: StaticStorageRegistrationRelocationFailureV1,
) -> Result<T, StrongStaticStorageRegistrationValidationError> {
    Err(relocation_error_value(plan, role, kind))
}

fn relocation_error_value(
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
    kind: StaticStorageRegistrationRelocationFailureV1,
) -> StrongStaticStorageRegistrationValidationError {
    StrongStaticStorageRegistrationValidationError::RelocationMismatch {
        storage: plan.semantic().storage(),
        role,
        kind,
    }
}
