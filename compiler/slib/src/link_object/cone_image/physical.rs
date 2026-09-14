use std::collections::{BTreeMap, BTreeSet};

use scoop_wire::sha256;

use super::{ConeImageAtomFileRangeFailureV1, ConeImageValidationError};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1,
};

pub(super) fn validate_objects<'a>(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &'a [ScoopLirObjectCandidateV1<'a>],
) -> Result<BTreeMap<SlibMemberId, &'a [u8]>, ConeImageValidationError> {
    for (index, pair) in objects.windows(2).enumerate() {
        if pair[0].member() >= pair[1].member() {
            return Err(if pair[0].member() == pair[1].member() {
                ConeImageValidationError::DuplicateObjectMember(pair[0].member())
            } else {
                ConeImageValidationError::NonCanonicalObjectOrder { index: index + 1 }
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
        return Err(ConeImageValidationError::UnexpectedObjectMember(*member));
    }
    if let Some(member) = expected.difference(&actual).next() {
        return Err(ConeImageValidationError::MissingObjectMember(*member));
    }

    let mut verified = BTreeMap::new();
    for object in objects {
        let member = verified_member(builtins, object.member())?;
        let envelope = member.definitions().sections().envelope();
        if u64::try_from(object.bytes().len()).ok() != Some(envelope.byte_length())
            || sha256(object.bytes()) != envelope.content_digest()
        {
            return Err(ConeImageValidationError::ObjectBytesMismatch(
                object.member(),
            ));
        }
        verified.insert(object.member(), object.bytes());
    }
    Ok(verified)
}

pub(super) fn verified_member(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    member: SlibMemberId,
) -> Result<&VerifiedMemberObjectRelocationIndexV1, ConeImageValidationError> {
    builtins
        .strong_relocations()
        .members()
        .binary_search_by_key(&member, |item| item.member())
        .ok()
        .map(|index| &builtins.strong_relocations().members()[index])
        .ok_or(ConeImageValidationError::MissingVerifiedMember(member))
}

pub(super) fn atom_file_range(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: VerifiedDefinitionAtomRangeV1,
) -> Result<(BuiltinObjectSectionRoleV1, u64, u64), ConeImageAtomFileRangeFailureV1> {
    let index = usize::from(atom.section_ordinal().get()) - 1;
    let section = member
        .definitions()
        .sections()
        .envelope()
        .sections()
        .get(index)
        .copied()
        .ok_or(ConeImageAtomFileRangeFailureV1::MissingSection)?;
    let role = *member
        .definitions()
        .sections()
        .roles()
        .get(index)
        .ok_or(ConeImageAtomFileRangeFailureV1::MissingSection)?;
    let file_offset = section
        .file_offset()
        .ok_or(ConeImageAtomFileRangeFailureV1::NotFileBacked)?;
    let start = atom
        .start()
        .checked_sub(section.virtual_address())
        .ok_or(ConeImageAtomFileRangeFailureV1::InvalidRange)?;
    let end = atom
        .end()
        .checked_sub(section.virtual_address())
        .ok_or(ConeImageAtomFileRangeFailureV1::InvalidRange)?;
    if start > end || end > section.byte_size() {
        return Err(ConeImageAtomFileRangeFailureV1::InvalidRange);
    }
    Ok((
        role,
        file_offset
            .checked_add(start)
            .ok_or(ConeImageAtomFileRangeFailureV1::InvalidRange)?,
        file_offset
            .checked_add(end)
            .ok_or(ConeImageAtomFileRangeFailureV1::InvalidRange)?,
    ))
}
