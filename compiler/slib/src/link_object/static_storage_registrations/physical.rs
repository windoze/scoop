use std::collections::{BTreeMap, BTreeSet};

use scoop_wire::sha256;

use super::{StaticStorageAtomRangeFailureV1, StrongStaticStorageRegistrationValidationError};
use crate::SlibMemberId;
use crate::link_object::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1,
};

pub(super) fn validate_objects<'a>(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &'a [ScoopLirObjectCandidateV1<'a>],
) -> Result<BTreeMap<SlibMemberId, &'a [u8]>, StrongStaticStorageRegistrationValidationError> {
    for (index, pair) in objects.windows(2).enumerate() {
        if pair[0].member() >= pair[1].member() {
            return Err(if pair[0].member() == pair[1].member() {
                StrongStaticStorageRegistrationValidationError::DuplicateObjectMember(
                    pair[0].member(),
                )
            } else {
                StrongStaticStorageRegistrationValidationError::NonCanonicalObjectOrder {
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
            StrongStaticStorageRegistrationValidationError::UnexpectedObjectMember(*member),
        );
    }
    if let Some(member) = expected.difference(&actual).next() {
        return Err(StrongStaticStorageRegistrationValidationError::MissingObjectMember(*member));
    }

    let mut verified = BTreeMap::new();
    for object in objects {
        let member = verified_member(builtins, object.member())?;
        let envelope = member.definitions().sections().envelope();
        if u64::try_from(object.bytes().len()).ok() != Some(envelope.byte_length())
            || sha256(object.bytes()) != envelope.content_digest()
        {
            return Err(
                StrongStaticStorageRegistrationValidationError::ObjectBytesMismatch(
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
) -> Result<&VerifiedMemberObjectRelocationIndexV1, StrongStaticStorageRegistrationValidationError>
{
    builtins
        .strong_relocations()
        .members()
        .binary_search_by_key(&member, |item| item.member())
        .ok()
        .map(|index| &builtins.strong_relocations().members()[index])
        .ok_or(StrongStaticStorageRegistrationValidationError::MissingVerifiedMember(member))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StaticStorageAtomRangeV1 {
    FileBacked {
        role: BuiltinObjectSectionRoleV1,
        start: u64,
        end: u64,
    },
    ZeroFill {
        byte_size: u64,
    },
}

impl StaticStorageAtomRangeV1 {
    pub(super) const fn role(self) -> BuiltinObjectSectionRoleV1 {
        match self {
            Self::FileBacked { role, .. } => role,
            Self::ZeroFill { .. } => BuiltinObjectSectionRoleV1::ZeroFill,
        }
    }

    pub(super) const fn byte_size(self) -> u64 {
        match self {
            Self::FileBacked { start, end, .. } => end - start,
            Self::ZeroFill { byte_size } => byte_size,
        }
    }
}

pub(super) fn atom_range(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: VerifiedDefinitionAtomRangeV1,
) -> Result<StaticStorageAtomRangeV1, StaticStorageAtomRangeFailureV1> {
    let index = usize::from(atom.section_ordinal().get()) - 1;
    let section = member
        .definitions()
        .sections()
        .envelope()
        .sections()
        .get(index)
        .copied()
        .ok_or(StaticStorageAtomRangeFailureV1::MissingSection)?;
    let role = *member
        .definitions()
        .sections()
        .roles()
        .get(index)
        .ok_or(StaticStorageAtomRangeFailureV1::MissingSection)?;
    let start_in_section = atom
        .start()
        .checked_sub(section.virtual_address())
        .ok_or(StaticStorageAtomRangeFailureV1::InvalidRange)?;
    let end_in_section = atom
        .end()
        .checked_sub(section.virtual_address())
        .ok_or(StaticStorageAtomRangeFailureV1::InvalidRange)?;
    if end_in_section > section.byte_size() || start_in_section > end_in_section {
        return Err(StaticStorageAtomRangeFailureV1::InvalidRange);
    }
    let byte_size = end_in_section - start_in_section;
    let Some(section_file_offset) = section.file_offset() else {
        return if role == BuiltinObjectSectionRoleV1::ZeroFill {
            Ok(StaticStorageAtomRangeV1::ZeroFill { byte_size })
        } else {
            Err(StaticStorageAtomRangeFailureV1::NotFileBacked)
        };
    };
    if role == BuiltinObjectSectionRoleV1::ZeroFill {
        return Err(StaticStorageAtomRangeFailureV1::InvalidRange);
    }
    let start = section_file_offset
        .checked_add(start_in_section)
        .ok_or(StaticStorageAtomRangeFailureV1::InvalidRange)?;
    let end = section_file_offset
        .checked_add(end_in_section)
        .ok_or(StaticStorageAtomRangeFailureV1::InvalidRange)?;
    Ok(StaticStorageAtomRangeV1::FileBacked { role, start, end })
}

pub(super) fn atom_file_range(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: VerifiedDefinitionAtomRangeV1,
) -> Result<(BuiltinObjectSectionRoleV1, u64, u64), StaticStorageAtomRangeFailureV1> {
    match atom_range(member, atom)? {
        StaticStorageAtomRangeV1::FileBacked { role, start, end } => Ok((role, start, end)),
        StaticStorageAtomRangeV1::ZeroFill { .. } => {
            Err(StaticStorageAtomRangeFailureV1::NotFileBacked)
        }
    }
}
