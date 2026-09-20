use super::*;

pub(super) fn verify_descriptor<D, C>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    plan: &StrongTypeRegistrationPlan<D, C>,
    plans_by_exact: &BTreeMap<PersistentExactTypeId, &StrongTypeRegistrationPlan<D, C>>,
) -> Result<VerifiedStrongTypeDescriptorV1, StrongTypeRegistrationValidationError>
where
    D: LinkDescriptorReference,
{
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

fn verify_itable_directory<D, C>(
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

fn verify_itable_directory_pointer<D, C>(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
    directory_atom: scoop_identity::ObjectDefinitionAtomId,
    directory_section: std::num::NonZeroU8,
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

fn interface_target_matches<D, C>(
    shape: &VerifiedDarwinArm64RelocationShapeV1,
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
            matches!(
                shape,
                VerifiedDarwinArm64RelocationShapeV1::Unsigned64 {
                    target: VerifiedRelocationTargetV1::StrongDefinition { definition }
                } if *definition == expected.descriptor_definition_plan()
            )
        }
        DescriptorReferenceKind::External(exact_type) => {
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

fn verify_descriptor_diagnostic_relocation<D: Copy, C>(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
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
