use super::*;
use crate::link_object::{
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1, VerifiedStrongObjectDefinitionV1,
};

pub(super) fn ranges(
    definition: &VerifiedStrongObjectDefinitionV1,
) -> Vec<VerifiedDefinitionAtomRangeV1> {
    let mut ranges: Vec<_> = definition
        .atoms()
        .iter()
        .filter(|atom| match atom.atom_role() {
            DefinitionAtomRole::Lsda
            | DefinitionAtomRole::EhFrame
            | DefinitionAtomRole::CompactUnwind
            | DefinitionAtomRole::Stackmap
            | DefinitionAtomRole::AddressTakenConstant => true,
            DefinitionAtomRole::Primary | DefinitionAtomRole::RuntimeRecord => false,
        })
        .copied()
        .collect();
    ranges.sort_unstable_by_key(|range| range.atom());
    ranges
}

pub(super) fn fingerprint_atoms(
    object: &[u8],
    member: &VerifiedMemberObjectRelocationIndexV1,
    ranges: &[VerifiedDefinitionAtomRangeV1],
    associated: &[VerifiedDefinitionAtomRangeV1],
    closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
    body: PersistentCallableBodyId,
) -> Result<Vec<AssociatedFingerprintAtom>, StrongCallableBodyFingerprintError> {
    let mut output = Vec::with_capacity(ranges.len());
    for range in ranges {
        let invalid = || StrongCallableBodyFingerprintError::InvalidAssociatedAtom {
            body,
            atom: range.atom(),
        };
        let (section, start, end) = atom_file_range(member, *range).map_err(|_| invalid())?;
        let correct_section = match range.atom_role() {
            DefinitionAtomRole::Lsda => section == BuiltinObjectSectionRoleV1::GccExceptionTable,
            DefinitionAtomRole::EhFrame => section == BuiltinObjectSectionRoleV1::EhFrame,
            DefinitionAtomRole::CompactUnwind => {
                section == BuiltinObjectSectionRoleV1::CompactUnwind
            }
            DefinitionAtomRole::Stackmap => section == BuiltinObjectSectionRoleV1::LlvmStackmaps,
            DefinitionAtomRole::AddressTakenConstant => matches!(
                section,
                BuiltinObjectSectionRoleV1::ReadOnlyData | BuiltinObjectSectionRoleV1::CString
            ),
            DefinitionAtomRole::Primary | DefinitionAtomRole::RuntimeRecord => false,
        };
        if !correct_section {
            return Err(invalid());
        }
        let start = usize::try_from(start).map_err(|_| invalid())?;
        let end = usize::try_from(end).map_err(|_| invalid())?;
        let bytes = object
            .get(start..end)
            .filter(|bytes| !bytes.is_empty())
            .ok_or_else(invalid)?;
        let relocations = canonicalize_relocations_with_associated_atoms(
            bytes,
            member,
            range.atom(),
            closure,
            requirements,
            associated,
        )
        .map_err(|kind| StrongCallableBodyFingerprintError::Relocation { body, kind })?;
        let bytes = normalize_atom_bytes(bytes, &relocations)
            .map_err(|kind| StrongCallableBodyFingerprintError::Relocation { body, kind })?;
        output.push(AssociatedFingerprintAtom {
            atom: range.atom(),
            role: range.atom_role(),
            bytes,
            relocations,
        });
    }
    Ok(output)
}
