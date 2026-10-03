use std::collections::{BTreeMap, BTreeSet};

use scoop_wire::sha256;

use super::super::{
    BuiltinObjectSectionRoleV1, ScoopLirObjectCandidateV1,
    VerifiedBuiltinObjectStrongRelocationSetV1, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1,
};
use super::expected::ExpectedPatchSite;
use super::{
    AtomFileRangeFailure, DIGEST_SLOT_WIDTH, DigestPatchSiteValidationError,
    ProvisionalDigestPatchSiteV1, VerifiedMaterializedPatchSiteV1,
};
use crate::SlibMemberId;

pub(super) fn validate_scoop_objects<'a>(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &'a [ScoopLirObjectCandidateV1<'a>],
) -> Result<BTreeMap<SlibMemberId, &'a [u8]>, DigestPatchSiteValidationError> {
    let expected = builtins
        .member_plan()
        .scoop_lir_members()
        .iter()
        .map(|member| member.member_id())
        .collect::<Vec<_>>();
    let actual = objects
        .iter()
        .map(|object| object.member())
        .collect::<Vec<_>>();
    validate_canonical_members(&actual)?;
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    let actual_set = actual.iter().copied().collect::<BTreeSet<_>>();
    if let Some(member) = actual_set.difference(&expected_set).next() {
        return Err(DigestPatchSiteValidationError::UnexpectedObjectMember(
            *member,
        ));
    }
    if let Some(member) = expected_set.difference(&actual_set).next() {
        return Err(DigestPatchSiteValidationError::MissingObjectMember(*member));
    }

    let members = builtins.strong_relocations().members();
    let mut verified = BTreeMap::new();
    for object in objects {
        let member = members
            .binary_search_by_key(&object.member(), |member| member.member())
            .ok()
            .map(|index| &members[index])
            .ok_or(DigestPatchSiteValidationError::MissingVerifiedObjectMember(
                object.member(),
            ))?;
        let envelope = member.definitions().sections().envelope();
        if u64::try_from(object.bytes().len()).ok() != Some(envelope.byte_length())
            || sha256(object.bytes()) != envelope.content_digest()
        {
            return Err(DigestPatchSiteValidationError::ObjectBytesMismatch(
                object.member(),
            ));
        }
        verified.insert(object.member(), object.bytes());
    }
    Ok(verified)
}

fn validate_canonical_members(
    members: &[SlibMemberId],
) -> Result<(), DigestPatchSiteValidationError> {
    for (index, pair) in members.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(if pair[0] == pair[1] {
                DigestPatchSiteValidationError::DuplicateObjectMember(pair[0])
            } else {
                DigestPatchSiteValidationError::NonCanonicalObjectMemberOrder { index: index + 1 }
            });
        }
    }
    Ok(())
}

pub(super) fn verify_site(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &BTreeMap<SlibMemberId, &[u8]>,
    expected: &ExpectedPatchSite,
    provisional: ProvisionalDigestPatchSiteV1,
) -> Result<VerifiedMaterializedPatchSiteV1, DigestPatchSiteValidationError> {
    if provisional.member != expected.member {
        return Err(DigestPatchSiteValidationError::PatchMemberMismatch {
            intent: expected.intent,
            expected: expected.member,
            actual: provisional.member,
        });
    }
    if provisional.width_bytes != DIGEST_SLOT_WIDTH {
        return Err(DigestPatchSiteValidationError::PatchWidthMismatch {
            intent: expected.intent,
            actual: provisional.width_bytes,
        });
    }
    let member = verified_member(builtins, expected.member)?;
    let definition = member.definitions().definition(expected.definition).ok_or(
        DigestPatchSiteValidationError::MissingVerifiedDefinition {
            intent: expected.intent,
            definition: expected.definition,
        },
    )?;
    let atom = definition
        .atoms()
        .iter()
        .find(|atom| atom.atom() == expected.atom && atom.atom_role() == expected.atom_role)
        .copied()
        .ok_or(DigestPatchSiteValidationError::MissingVerifiedAtom {
            intent: expected.intent,
            atom: expected.atom,
        })?;
    let (section_role, atom_file_start, atom_file_end) =
        atom_file_range(member, atom).map_err(|kind| {
            DigestPatchSiteValidationError::InvalidAtomFileRange {
                intent: expected.intent,
                atom: expected.atom,
                kind,
            }
        })?;
    let site_end = provisional
        .checked_offset
        .checked_add(u64::from(DIGEST_SLOT_WIDTH))
        .ok_or(DigestPatchSiteValidationError::PatchOffsetOverflow(
            expected.intent,
        ))?;
    if provisional.checked_offset < atom_file_start || site_end > atom_file_end {
        return Err(DigestPatchSiteValidationError::PatchOutsideAtom {
            intent: expected.intent,
            atom: expected.atom,
            atom_start: atom_file_start,
            atom_end: atom_file_end,
            patch_start: provisional.checked_offset,
            patch_end: site_end,
        });
    }
    let bytes = objects[&expected.member];
    let start = usize::try_from(provisional.checked_offset)
        .map_err(|_| DigestPatchSiteValidationError::PatchOffsetOverflow(expected.intent))?;
    let end = usize::try_from(site_end)
        .map_err(|_| DigestPatchSiteValidationError::PatchOffsetOverflow(expected.intent))?;
    let slot = bytes
        .get(start..end)
        .ok_or(DigestPatchSiteValidationError::PatchOffsetOverflow(
            expected.intent,
        ))?;
    if let Some(relative) = slot.iter().position(|byte| *byte != 0) {
        return Err(DigestPatchSiteValidationError::NonZeroProvisionalSlot {
            intent: expected.intent,
            byte_offset: provisional.checked_offset + relative as u64,
        });
    }
    let offset_within_atom = provisional.checked_offset - atom_file_start;
    reject_overlapping_relocations(
        member,
        expected,
        atom_file_start,
        provisional.checked_offset,
        site_end,
    )?;
    Ok(VerifiedMaterializedPatchSiteV1 {
        intent: expected.intent,
        source: expected.source,
        semantic_field_role: expected.semantic_field_role,
        member: expected.member,
        definition: expected.definition,
        atom: expected.atom,
        atom_role: expected.atom_role,
        section_role,
        checked_offset: provisional.checked_offset,
        offset_within_atom,
    })
}

fn reject_overlapping_relocations(
    member: &VerifiedMemberObjectRelocationIndexV1,
    expected: &ExpectedPatchSite,
    atom_file_start: u64,
    site_start: u64,
    site_end: u64,
) -> Result<(), DigestPatchSiteValidationError> {
    for relocation in member.relocations() {
        if relocation.containing_atom() != expected.atom {
            continue;
        }
        let relocation_start = atom_file_start
            .checked_add(relocation.offset_within_atom())
            .ok_or(DigestPatchSiteValidationError::PatchOffsetOverflow(
                expected.intent,
            ))?;
        let relocation_end = relocation_start
            .checked_add(u64::from(relocation.width_bytes()))
            .ok_or(DigestPatchSiteValidationError::PatchOffsetOverflow(
                expected.intent,
            ))?;
        if ranges_overlap(site_start, site_end, relocation_start, relocation_end) {
            return Err(DigestPatchSiteValidationError::RelocationOverlapsPatch {
                intent: expected.intent,
                relocation_offset: relocation.offset_within_atom(),
            });
        }
    }
    Ok(())
}

fn verified_member(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    member: SlibMemberId,
) -> Result<&VerifiedMemberObjectRelocationIndexV1, DigestPatchSiteValidationError> {
    let members = builtins.strong_relocations().members();
    members
        .binary_search_by_key(&member, |item| item.member())
        .ok()
        .map(|index| &members[index])
        .ok_or(DigestPatchSiteValidationError::MissingVerifiedObjectMember(
            member,
        ))
}

fn atom_file_range(
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: VerifiedDefinitionAtomRangeV1,
) -> Result<(BuiltinObjectSectionRoleV1, u64, u64), AtomFileRangeFailure> {
    let index = usize::from(atom.section_ordinal().get()) - 1;
    let section = member
        .definitions()
        .sections()
        .envelope()
        .sections()
        .get(index)
        .copied()
        .ok_or(AtomFileRangeFailure::MissingSection)?;
    let role = *member
        .definitions()
        .sections()
        .roles()
        .get(index)
        .ok_or(AtomFileRangeFailure::MissingSection)?;
    let file_offset = section
        .file_offset()
        .ok_or(AtomFileRangeFailure::NotFileBacked)?;
    let start_in_section = atom
        .start()
        .checked_sub(section.virtual_address())
        .ok_or(AtomFileRangeFailure::InvalidRange)?;
    let end_in_section = atom
        .end()
        .checked_sub(section.virtual_address())
        .ok_or(AtomFileRangeFailure::InvalidRange)?;
    let start = file_offset
        .checked_add(start_in_section)
        .ok_or(AtomFileRangeFailure::InvalidRange)?;
    let end = file_offset
        .checked_add(end_in_section)
        .ok_or(AtomFileRangeFailure::InvalidRange)?;
    Ok((role, start, end))
}

pub(super) fn validate_disjoint_sites(
    sites: &[VerifiedMaterializedPatchSiteV1],
) -> Result<(), DigestPatchSiteValidationError> {
    let mut by_location = sites.to_vec();
    by_location.sort_unstable_by_key(|site| (site.member, site.checked_offset, site.intent));
    for pair in by_location.windows(2) {
        if pair[0].member == pair[1].member
            && ranges_overlap(
                pair[0].checked_offset,
                pair[0].checked_offset + u64::from(DIGEST_SLOT_WIDTH),
                pair[1].checked_offset,
                pair[1].checked_offset + u64::from(DIGEST_SLOT_WIDTH),
            )
        {
            return Err(DigestPatchSiteValidationError::OverlappingPatchSites {
                first: pair[0].intent,
                second: pair[1].intent,
            });
        }
    }
    Ok(())
}

const fn ranges_overlap(
    first_start: u64,
    first_end: u64,
    second_start: u64,
    second_end: u64,
) -> bool {
    first_start < second_end && second_start < first_end
}
