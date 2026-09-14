use std::collections::{BTreeMap, BTreeSet};

use scoop_wire::sha256;

use super::{
    SafepointRegistrationAtomFileRangeFailureV1, StrongSafepointRegistrationValidationError,
};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1,
};

pub(in crate::link_object) fn validate_objects<'a>(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &'a [ScoopLirObjectCandidateV1<'a>],
) -> Result<BTreeMap<SlibMemberId, &'a [u8]>, StrongSafepointRegistrationValidationError> {
    for (index, pair) in objects.windows(2).enumerate() {
        if pair[0].member() >= pair[1].member() {
            return Err(if pair[0].member() == pair[1].member() {
                StrongSafepointRegistrationValidationError::DuplicateObjectMember(pair[0].member())
            } else {
                StrongSafepointRegistrationValidationError::NonCanonicalObjectOrder {
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
        return Err(StrongSafepointRegistrationValidationError::UnexpectedObjectMember(*member));
    }
    if let Some(member) = expected.difference(&actual).next() {
        return Err(StrongSafepointRegistrationValidationError::MissingObjectMember(*member));
    }

    let mut verified = BTreeMap::new();
    for object in objects {
        let member = verified_member(builtins, object.member())?;
        let envelope = member.definitions().sections().envelope();
        if u64::try_from(object.bytes().len()).ok() != Some(envelope.byte_length())
            || sha256(object.bytes()) != envelope.content_digest()
        {
            return Err(
                StrongSafepointRegistrationValidationError::ObjectBytesMismatch(object.member()),
            );
        }
        verified.insert(object.member(), object.bytes());
    }
    Ok(verified)
}

pub(in crate::link_object) fn verified_member(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    member: SlibMemberId,
) -> Result<&VerifiedMemberObjectRelocationIndexV1, StrongSafepointRegistrationValidationError> {
    builtins
        .strong_relocations()
        .members()
        .binary_search_by_key(&member, |item| item.member())
        .ok()
        .map(|index| &builtins.strong_relocations().members()[index])
        .ok_or(StrongSafepointRegistrationValidationError::MissingVerifiedMember(member))
}

pub(in crate::link_object) fn atom_file_range(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: VerifiedDefinitionAtomRangeV1,
) -> Result<(BuiltinObjectSectionRoleV1, u64, u64), SafepointRegistrationAtomFileRangeFailureV1> {
    let index = usize::from(atom.section_ordinal().get()) - 1;
    let section = member
        .definitions()
        .sections()
        .envelope()
        .sections()
        .get(index)
        .copied()
        .ok_or(SafepointRegistrationAtomFileRangeFailureV1::MissingSection)?;
    let role = *member
        .definitions()
        .sections()
        .roles()
        .get(index)
        .ok_or(SafepointRegistrationAtomFileRangeFailureV1::MissingSection)?;
    let section_file_offset = section
        .file_offset()
        .ok_or(SafepointRegistrationAtomFileRangeFailureV1::NotFileBacked)?;
    let start_in_section = atom
        .start()
        .checked_sub(section.virtual_address())
        .ok_or(SafepointRegistrationAtomFileRangeFailureV1::InvalidRange)?;
    let end_in_section = atom
        .end()
        .checked_sub(section.virtual_address())
        .ok_or(SafepointRegistrationAtomFileRangeFailureV1::InvalidRange)?;
    if end_in_section > section.byte_size() || start_in_section > end_in_section {
        return Err(SafepointRegistrationAtomFileRangeFailureV1::InvalidRange);
    }
    let start = section_file_offset
        .checked_add(start_in_section)
        .ok_or(SafepointRegistrationAtomFileRangeFailureV1::InvalidRange)?;
    let end = section_file_offset
        .checked_add(end_in_section)
        .ok_or(SafepointRegistrationAtomFileRangeFailureV1::InvalidRange)?;
    Ok((role, start, end))
}
