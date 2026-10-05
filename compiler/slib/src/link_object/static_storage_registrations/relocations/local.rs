use super::*;
use crate::link_object::VerifiedRelocationTargetV1;

pub(super) fn validate_local_atom_target(
    relocation: &VerifiedRelocationUseV1,
    expected_atom: ObjectDefinitionAtomId,
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    use StaticStorageRegistrationRelocationFailureV1 as Failure;
    let (section, address) = relocation
        .shape()
        .absolute64_local_address(relocation.encoded_value())
        .ok_or_else(|| relocation_error_value(plan, role, Failure::TargetKind))?;
    let expected = member
        .definitions()
        .definitions()
        .iter()
        .flat_map(|definition| definition.atoms())
        .find(|atom| atom.atom() == expected_atom)
        .ok_or_else(|| relocation_error_value(plan, role, Failure::TargetAtom))?;
    if (section, address) != (expected.section_ordinal(), expected.start()) {
        return relocation_error(plan, role, Failure::TargetAtom);
    }
    if member
        .definitions()
        .sections()
        .roles()
        .get(section.get() as usize - 1)
        != Some(&BuiltinObjectSectionRoleV1::ReadOnlyData)
    {
        return relocation_error(plan, role, Failure::TargetSection);
    }
    Ok(())
}

pub(super) fn validate_sentinel_target(
    relocation: &VerifiedRelocationUseV1,
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &StrongStaticStorageRegistrationPlanV1,
    role: StaticStorageRelocationRoleV1,
) -> Result<(), StrongStaticStorageRegistrationValidationError> {
    let (_, section, target) = sentinel_target_key(member, relocation).ok_or_else(|| {
        relocation_error_value(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::TargetKind,
        )
    })?;
    let section_index = section as usize - 1;
    let sections = member.definitions().sections();
    if sections.roles().get(section_index) != Some(&BuiltinObjectSectionRoleV1::ReadOnlyData) {
        return relocation_error(
            plan,
            role,
            StaticStorageRegistrationRelocationFailureV1::TargetSection,
        );
    }
    let section = &sections.envelope().sections()[section_index];
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

pub(in crate::link_object::static_storage_registrations) fn sentinel_target_key(
    member: &VerifiedMemberObjectRelocationIndexV1,
    relocation: &VerifiedRelocationUseV1,
) -> Option<(crate::SlibMemberId, u32, u64)> {
    let (section, base) = match relocation.shape().absolute64_target()? {
        VerifiedRelocationTargetV1::SectionBase {
            section_ordinal, ..
        } => {
            let section = member
                .definitions()
                .sections()
                .envelope()
                .sections()
                .get(section_ordinal.get() as usize - 1)?;
            (*section_ordinal, section.virtual_address())
        }
        VerifiedRelocationTargetV1::LocalDefinition {
            owner_atom: None,
            section_ordinal,
            value,
            ..
        } => (*section_ordinal, *value),
        _ => return None,
    };
    Some((
        member.member(),
        section.get(),
        relocation
            .shape()
            .form()
            .absolute64_address(relocation.encoded_value(), base)?,
    ))
}
