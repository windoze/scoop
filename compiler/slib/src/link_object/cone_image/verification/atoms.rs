use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn require_atom(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    expected_atom: ObjectDefinitionAtomId,
    expected_atom_role: DefinitionAtomRole,
    role: ConeImageAtomRoleV1,
    expected_size: u64,
    required_alignment: u64,
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let atom = atoms
        .iter()
        .find(|atom| atom.atom() == expected_atom && atom.atom_role() == expected_atom_role)
        .copied()
        .ok_or(ConeImageValidationError::MissingAtom {
            role,
            atom: expected_atom,
        })?;
    let (section_role, start, end) = atom_file_range(member, atom).map_err(|kind| {
        ConeImageValidationError::InvalidAtomFileRange {
            role,
            atom: expected_atom,
            kind,
        }
    })?;
    if section_role != BuiltinObjectSectionRoleV1::ReadOnlyData {
        return Err(ConeImageValidationError::AtomSectionMismatch {
            role,
            actual: section_role,
        });
    }
    let actual_size = end - start;
    if actual_size != expected_size {
        return Err(ConeImageValidationError::AtomSizeMismatch {
            role,
            expected: expected_size,
            actual: actual_size,
        });
    }
    if atom.start() % required_alignment != 0 {
        return Err(ConeImageValidationError::AtomAlignmentMismatch {
            role,
            required: required_alignment,
            address: atom.start(),
        });
    }
    Ok(VerifiedConeImageAtomV1 {
        atom: expected_atom,
        checked_offset: start,
        byte_size: actual_size,
    })
}

pub(super) fn require_support_bytes(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    object: &[u8],
    atom: ObjectDefinitionAtomId,
    role: ConeImageAtomRoleV1,
    expected: &[u8],
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let verified = require_atom(
        member,
        atoms,
        atom,
        DefinitionAtomRole::AddressTakenConstant,
        role,
        expected.len() as u64,
        1,
    )?;
    validate_atom_bytes(object, verified, role, expected)?;
    require_relocation_count(member, atom, 0, role)?;
    Ok(verified)
}

pub(super) fn require_table_bytes(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atoms: &[VerifiedDefinitionAtomRangeV1],
    object: &[u8],
    atom: ObjectDefinitionAtomId,
    role: ConeImageAtomRoleV1,
    expected: &[u8],
    alignment: u64,
) -> Result<VerifiedConeImageAtomV1, ConeImageValidationError> {
    let verified = require_atom(
        member,
        atoms,
        atom,
        DefinitionAtomRole::RuntimeRecord,
        role,
        expected.len() as u64,
        alignment,
    )?;
    validate_atom_bytes(object, verified, role, expected)?;
    Ok(verified)
}

pub(super) fn validate_atom_bytes(
    object: &[u8],
    atom: VerifiedConeImageAtomV1,
    role: ConeImageAtomRoleV1,
    expected: &[u8],
) -> Result<(), ConeImageValidationError> {
    let start = usize::try_from(atom.checked_offset)
        .map_err(|_| ConeImageValidationError::RecordRangeOverflow(role))?;
    let end = start
        .checked_add(expected.len())
        .ok_or(ConeImageValidationError::RecordRangeOverflow(role))?;
    let actual = object
        .get(start..end)
        .ok_or(ConeImageValidationError::RecordRangeOverflow(role))?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(ConeImageValidationError::AtomByteMismatch {
            role,
            offset_within_atom: offset as u64,
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}
