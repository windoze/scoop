use super::*;

pub(super) fn verify_primary_relocations(
    member: &VerifiedMemberObjectRelocationIndexV1,
    plan: &ConeImagePlanV1,
    support: VerifiedConeImageSupportAtomsV1,
) -> Result<Vec<VerifiedRelocationUseV1>, ConeImageValidationError> {
    let expected = [
        (
            16,
            support.coordinate_group,
            ConeImageAtomRoleV1::CoordinateGroup,
        ),
        (
            32,
            support.coordinate_name,
            ConeImageAtomRoleV1::CoordinateName,
        ),
        (
            48,
            support.coordinate_version,
            ConeImageAtomRoleV1::CoordinateVersion,
        ),
        (128, support.dependencies, ConeImageAtomRoleV1::Dependencies),
        (
            144,
            support.static_storages,
            ConeImageAtomRoleV1::StaticStorages,
        ),
        (
            160,
            support.immortal_objects,
            ConeImageAtomRoleV1::ImmortalObjects,
        ),
        (
            176,
            support.initialization_units,
            ConeImageAtomRoleV1::InitializationUnits,
        ),
        (
            192,
            support.type_registrations,
            ConeImageAtomRoleV1::TypeRegistrations,
        ),
        (208, support.safepoints, ConeImageAtomRoleV1::Safepoints),
        (224, support.callables, ConeImageAtomRoleV1::Callables),
    ];
    require_relocation_count(
        member,
        plan.primary_atom(),
        expected.len(),
        ConeImageAtomRoleV1::Primary,
    )?;
    expected
        .into_iter()
        .enumerate()
        .map(|(index, (offset, target, role))| {
            verify_local_relocation(member, plan.primary_atom(), offset, target, role, index)
        })
        .collect()
}

fn verify_local_relocation(
    member: &VerifiedMemberObjectRelocationIndexV1,
    source_atom: ObjectDefinitionAtomId,
    offset: u64,
    target_atom: VerifiedConeImageAtomV1,
    role: ConeImageAtomRoleV1,
    index: usize,
) -> Result<VerifiedRelocationUseV1, ConeImageValidationError> {
    let relocation = require_relocation(member, source_atom, offset, role, index)?;
    let kind = if relocation.containing_atom_role() != DefinitionAtomRole::Primary {
        Some(ConeImageRelocationFailureV1::ContainingAtomRole)
    } else if relocation.section_role() != BuiltinObjectSectionRoleV1::ReadOnlyData {
        Some(ConeImageRelocationFailureV1::SectionRole)
    } else if relocation.width_bytes() != 8 {
        Some(ConeImageRelocationFailureV1::Width)
    } else if !relocation.shape().form().is_absolute64() {
        Some(ConeImageRelocationFailureV1::Form)
    } else {
        let target_range = member
            .definitions()
            .definitions()
            .iter()
            .flat_map(|definition| definition.atoms())
            .find(|atom| atom.atom() == target_atom.atom)
            .expect("verified image support atom remains in its definition");
        match relocation
            .shape()
            .absolute64_local_address(relocation.encoded_value())
        {
            Some((section, _)) if section != target_range.section_ordinal() => {
                Some(ConeImageRelocationFailureV1::TargetSection)
            }
            Some((_, value)) if value != target_range.start() => {
                Some(ConeImageRelocationFailureV1::TargetValue)
            }
            Some(_) => None,
            None => Some(ConeImageRelocationFailureV1::TargetKind),
        }
    };
    if let Some(kind) = kind {
        return relocation_error(role, index, kind);
    }
    Ok(relocation.clone())
}

pub(super) fn require_relocation_count(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: ObjectDefinitionAtomId,
    expected: usize,
    role: ConeImageAtomRoleV1,
) -> Result<(), ConeImageValidationError> {
    let actual = member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == atom)
        .count();
    if actual != expected {
        return relocation_error(role, actual, ConeImageRelocationFailureV1::Count);
    }
    Ok(())
}

pub(super) fn require_relocation(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: ObjectDefinitionAtomId,
    offset: u64,
    role: ConeImageAtomRoleV1,
    index: usize,
) -> Result<&VerifiedRelocationUseV1, ConeImageValidationError> {
    let matches = member
        .relocations()
        .iter()
        .filter(|relocation| {
            relocation.containing_atom() == atom && relocation.offset_within_atom() == offset
        })
        .collect::<Vec<_>>();
    let [relocation] = matches.as_slice() else {
        return relocation_error(role, index, ConeImageRelocationFailureV1::MissingOffset);
    };
    Ok(*relocation)
}
