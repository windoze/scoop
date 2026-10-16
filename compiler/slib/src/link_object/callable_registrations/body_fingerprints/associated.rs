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
            | DefinitionAtomRole::AddressTakenConstant
            | DefinitionAtomRole::ContextKeyCell
            | DefinitionAtomRole::ContextKeyTable
            | DefinitionAtomRole::RuntimeRecord => true,
            DefinitionAtomRole::Primary => false,
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
        let (_, start, end) = atom_file_range(member, *range).map_err(|_| invalid())?;
        let start = usize::try_from(start).map_err(|_| invalid())?;
        let end = usize::try_from(end).map_err(|_| invalid())?;
        let bytes = object.get(start..end).ok_or_else(invalid)?;
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
