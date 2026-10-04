use std::collections::{BTreeMap, BTreeSet};

use scoop_wire::sha256;

use super::{
    InitializationAtomFileRangeFailureV1, StrongInitializationRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1,
};

pub(super) fn validate_objects<'a>(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &'a [ScoopLirObjectCandidateV1<'a>],
) -> Result<BTreeMap<SlibMemberId, &'a [u8]>, StrongInitializationRegistrationValidationError> {
    for (index, pair) in objects.windows(2).enumerate() {
        if pair[0].member() >= pair[1].member() {
            return Err(if pair[0].member() == pair[1].member() {
                StrongInitializationRegistrationValidationError::DuplicateObjectMember(
                    pair[0].member(),
                )
            } else {
                StrongInitializationRegistrationValidationError::NonCanonicalObjectOrder {
                    index: index + 1,
                }
            });
        }
    }
    let expected = builtins
        .member_plan()
        .scoop_lir_members()
        .iter()
        .map(|member| member.member_id())
        .collect::<BTreeSet<_>>();
    let actual = objects
        .iter()
        .map(|object| object.member())
        .collect::<BTreeSet<_>>();
    if let Some(member) = actual.difference(&expected).next() {
        return Err(
            StrongInitializationRegistrationValidationError::UnexpectedObjectMember(*member),
        );
    }
    if let Some(member) = expected.difference(&actual).next() {
        return Err(StrongInitializationRegistrationValidationError::MissingObjectMember(*member));
    }

    let mut verified = BTreeMap::new();
    for object in objects {
        let member = verified_member(builtins, object.member())?;
        let envelope = member.definitions().sections().envelope();
        if u64::try_from(object.bytes().len()).ok() != Some(envelope.byte_length())
            || sha256(object.bytes()) != envelope.content_digest()
        {
            return Err(
                StrongInitializationRegistrationValidationError::ObjectBytesMismatch(
                    object.member(),
                ),
            );
        }
        verified.insert(object.member(), object.bytes());
    }
    Ok(verified)
}

pub(super) fn verified_member(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    member: SlibMemberId,
) -> Result<&VerifiedMemberObjectRelocationIndexV1, StrongInitializationRegistrationValidationError>
{
    builtins
        .strong_relocations()
        .members()
        .binary_search_by_key(&member, |item| item.member())
        .ok()
        .map(|index| &builtins.strong_relocations().members()[index])
        .ok_or(StrongInitializationRegistrationValidationError::MissingVerifiedMember(member))
}

pub(super) fn atom_file_range(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: VerifiedDefinitionAtomRangeV1,
) -> Result<(BuiltinObjectSectionRoleV1, u64, u64), InitializationAtomFileRangeFailureV1> {
    let index = (atom.section_ordinal().get() as usize) - 1;
    let section = member
        .definitions()
        .sections()
        .envelope()
        .sections()
        .get(index)
        .ok_or(InitializationAtomFileRangeFailureV1::MissingSection)?;
    let role = *member
        .definitions()
        .sections()
        .roles()
        .get(index)
        .ok_or(InitializationAtomFileRangeFailureV1::MissingSection)?;
    let file_offset = section
        .file_offset()
        .ok_or(InitializationAtomFileRangeFailureV1::NotFileBacked)?;
    let start = atom
        .start()
        .checked_sub(section.virtual_address())
        .ok_or(InitializationAtomFileRangeFailureV1::InvalidRange)?;
    let end = atom
        .end()
        .checked_sub(section.virtual_address())
        .ok_or(InitializationAtomFileRangeFailureV1::InvalidRange)?;
    if end > section.byte_size() || start > end {
        return Err(InitializationAtomFileRangeFailureV1::InvalidRange);
    }
    Ok((
        role,
        file_offset
            .checked_add(start)
            .ok_or(InitializationAtomFileRangeFailureV1::InvalidRange)?,
        file_offset
            .checked_add(end)
            .ok_or(InitializationAtomFileRangeFailureV1::InvalidRange)?,
    ))
}
