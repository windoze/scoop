use super::*;

pub(super) fn verify_itable_directory<D, C>(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    definition: &crate::link_object::VerifiedStrongObjectDefinitionV1,
    object: &[u8],
    plan: &StrongTypeRegistrationPlan<D, C>,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlan<D, C>>,
) -> Result<VerifiedTypeDescriptorITableDirectoryV1, StrongTypeRegistrationValidationError>
where
    D: LinkDescriptorReference,
{
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

    let descriptor_relocation =
        verify_itable_directory_pointer(member, plan, atom.section_ordinal(), atom.start())?;
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
        if !interface_target_matches(
            builtins,
            member.member(),
            interface,
            itable.interface(),
            plans_by_exact,
        ) {
            return itable_directory_error(plan, Some(entry_index), Failure::InterfaceTarget);
        }
        if !itable.slots().is_empty() {
            let slots = &relocations[relocation_index];
            relocation_index += 1;
            validate_itable_relocation_shape(plan, entry_index, slots)?;
            let expected =
                dispatch_definition(builtins.producer(), plan.definition_owner(), itable.table())
                    .map_err(|_| {
                    itable_directory_failure(
                        plan,
                        Some(entry_index),
                        Failure::DispatchDefinitionIdentity,
                    )
                })?;
            if !definition_target_matches(builtins, member.member(), slots, expected) {
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

fn verify_itable_directory_pointer<D, C>(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
    directory_section: std::num::NonZeroU32,
    directory_value: u64,
) -> Result<VerifiedRelocationUseV1, StrongTypeRegistrationValidationError>
where
    D: LinkDescriptorReference,
{
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
        && relocation
            .shape()
            .absolute64_local_address(relocation.encoded_value())
            == Some((directory_section, directory_value));
    if !matches {
        return itable_directory_error(plan, None, Failure::DescriptorRelocation);
    }
    Ok((*relocation).clone())
}

fn validate_itable_relocation_shape<D, C>(
    plan: &StrongTypeRegistrationPlan<D, C>,
    entry: usize,
    relocation: &VerifiedRelocationUseV1,
) -> Result<(), StrongTypeRegistrationValidationError>
where
    D: LinkDescriptorReference,
{
    use TypeDescriptorITableDirectoryFailureV1 as Failure;
    if relocation.containing_atom_role() != DefinitionAtomRole::RuntimeRecord
        || relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData
        || relocation.width_bytes() != 8
        || relocation
            .shape()
            .form()
            .absolute64_addend(relocation.encoded_value())
            != Some(0)
    {
        return itable_directory_error(plan, Some(entry), Failure::RelocationShape);
    }
    Ok(())
}

fn interface_target_matches<D, C>(
    builtins: &crate::VerifiedBuiltinObjectStrongRelocationSetV1,
    source_member: SlibMemberId,
    relocation: &VerifiedRelocationUseV1,
    interface: D,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlan<D, C>>,
) -> bool
where
    D: LinkDescriptorReference,
{
    match interface.kind() {
        DescriptorReferenceKind::Local(exact_type) => {
            let Some(expected) = plans_by_exact.get(&exact_type) else {
                return false;
            };
            definition_target_matches(
                builtins,
                source_member,
                relocation,
                expected.descriptor_definition_plan(),
            )
        }
        DescriptorReferenceKind::External(exact_type) => {
            let expected =
                expected_type_descriptor_symbol(builtins.member_plan().target(), exact_type);
            matches!(
                relocation.shape().absolute64_target(),
                Some(VerifiedRelocationTargetV1::ExternalUndefined { name, .. }) if name == &expected
            )
        }
    }
}

fn definition_target_matches(
    builtins: &crate::VerifiedBuiltinObjectStrongRelocationSetV1,
    source_member: SlibMemberId,
    relocation: &VerifiedRelocationUseV1,
    expected: ObjectDefinitionPlanId,
) -> bool {
    super::super::targets::local_definition(builtins, source_member, relocation) == Some(expected)
}

fn dispatch_definition(
    producer: scoop_identity::ConeIdentity,
    owner: scoop_lir::RegistrationDefinitionOwner,
    table: scoop_identity::PersistentDispatchTableId,
) -> Result<ObjectDefinitionPlanId, ()> {
    let key = match owner {
        scoop_lir::RegistrationDefinitionOwner::Strong => ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::dispatch_table(table),
            StrongDefinitionRole::DispatchTable,
        )
        .map_err(|_| ())?,
        scoop_lir::RegistrationDefinitionOwner::Odr { group, .. } => {
            let member = scoop_identity::OdrMemberKey::new(
                group,
                scoop_identity::OdrMemberRole::DispatchTable,
                scoop_identity::OdrMemberDiscriminator::DispatchTable(table),
            )
            .map_err(|_| ())?;
            ObjectDefinitionPlanKey::odr(
                scoop_identity::OdrMemberId::from_key(&member).map_err(|_| ())?,
            )
        }
    };
    ObjectDefinitionPlanId::from_key(&key).map_err(|_| ())
}

fn expected_type_descriptor_symbol(
    target: scoop_lir::LirTargetProfile,
    exact_type: PersistentExactTypeId,
) -> Vec<u8> {
    let symbol = MangledSymbol::from_key(&PersistentSymbolKey::TypeDescriptor(exact_type));
    target
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(symbol.as_str())
        .into_bytes()
}

fn itable_directory_failure<D: Copy, C>(
    plan: &StrongTypeRegistrationPlan<D, C>,
    entry: Option<usize>,
    kind: TypeDescriptorITableDirectoryFailureV1,
) -> StrongTypeRegistrationValidationError {
    StrongTypeRegistrationValidationError::DescriptorITableDirectoryMismatch {
        exact_type: plan.exact_type(),
        entry,
        kind,
    }
}

fn itable_directory_error<T, D: Copy, C>(
    plan: &StrongTypeRegistrationPlan<D, C>,
    entry: Option<usize>,
    kind: TypeDescriptorITableDirectoryFailureV1,
) -> Result<T, StrongTypeRegistrationValidationError> {
    Err(itable_directory_failure(plan, entry, kind))
}
