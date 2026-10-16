use super::StrongTypeDependencyFingerprintError as Error;
use crate::link_object::{VerifiedMemberObjectRelocationIndexV1, atom_range::atom_file_range};
use scoop_identity::{ObjectDefinitionAtomId, ObjectDefinitionPlanId};

pub(super) fn atom_bytes<'a>(
    member: &VerifiedMemberObjectRelocationIndexV1,
    object: &'a [u8],
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
) -> Result<&'a [u8], Error> {
    let definition = member
        .definitions()
        .definition(definition)
        .ok_or(Error::MissingDefinition(definition))?;
    let range = definition
        .atoms()
        .iter()
        .find(|range| range.atom() == atom)
        .copied()
        .ok_or(Error::MissingAtom(atom))?;
    let (_, start, end) = atom_file_range(member, range).map_err(|_| Error::Range { atom })?;
    let start = usize::try_from(start).map_err(|_| Error::Range { atom })?;
    let end = usize::try_from(end).map_err(|_| Error::Range { atom })?;
    object.get(start..end).ok_or(Error::Range { atom })
}
