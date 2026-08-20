use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiagnosticTargetV1 {
    member: SlibMemberId,
    atom: ObjectDefinitionAtomId,
    section_ordinal: u32,
    value: u64,
}

pub(super) fn validate_diagnostic_target<D>(
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    member: &VerifiedMemberObjectRelocationIndexV1,
    relocation: &VerifiedRelocationUseV1,
    expected_target: DiagnosticTargetV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationRelocationRoleV1,
) -> Result<DiagnosticTargetV1, StrongInitializationRegistrationValidationError> {
    let (section_ordinal, value) = relocation
        .shape()
        .absolute64_local_address(relocation.encoded_value())
        .ok_or_else(|| {
            relocation_error_value(plan, role, InitializationRelocationFailureV1::TargetKind)
        })?;
    let section_index = (section_ordinal.get() as usize) - 1;
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

pub(super) fn expected_diagnostic_target<D>(
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<DiagnosticTargetV1, StrongInitializationRegistrationValidationError> {
    let definition = member
        .definitions()
        .definition(plan.registration_definition_plan())
        .ok_or(
            StrongInitializationRegistrationValidationError::MissingVerifiedDefinition {
                unit: plan.semantic().unit(),
                definition: plan.registration_definition_plan(),
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
    let section_index = (atom.section_ordinal().get() as usize) - 1;
    if member.definitions().sections().roles().get(section_index)
        != Some(&BuiltinObjectSectionRoleV1::CString)
    {
        return relocation_error(
            plan,
            InitializationRelocationRoleV1::RegistrationDiagnostic,
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
