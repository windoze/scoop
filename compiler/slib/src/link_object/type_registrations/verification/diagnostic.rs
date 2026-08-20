use super::*;

pub(super) fn verify_descriptor_diagnostic_relocation<D: Copy, C>(
    member: &crate::link_object::VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongTypeRegistrationPlan<D, C>,
    diagnostic_section: std::num::NonZeroU32,
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
    } else if !relocation.shape().form().is_absolute64() {
        Some(Failure::Form)
    } else {
        match relocation
            .shape()
            .absolute64_local_address(relocation.encoded_value())
        {
            Some((section, _)) if section != diagnostic_section => Some(Failure::TargetSection),
            Some((_, value)) if value != diagnostic_value => Some(Failure::TargetValue),
            Some(_) => None,
            None => Some(Failure::TargetKind),
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
