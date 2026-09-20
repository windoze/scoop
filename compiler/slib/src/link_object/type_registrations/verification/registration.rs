use super::*;

pub(super) fn required_scoop_member<D: Copy, C>(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
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

pub(super) fn verify_layout_definition<D: Copy, C>(
    builtins: &crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
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

pub(super) fn verify_descriptor_relocation<D: Copy, C>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    registration_member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
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
