use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::ProvisionalDigestPatchSiteV1;
use crate::SlibMemberId;
use crate::link_object::{PlannedLinkObjectMemberSetV1, ScoopLirObjectCandidateV1};

const DIGEST_WIDTH: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedNormalizedProvisionalScoopLirObjectV1 {
    member: SlibMemberId,
    final_bytes: Vec<u8>,
    provisional_bytes: Vec<u8>,
}

impl VerifiedNormalizedProvisionalScoopLirObjectV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub fn final_bytes(&self) -> &[u8] {
        &self.final_bytes
    }

    pub fn provisional_bytes(&self) -> &[u8] {
        &self.provisional_bytes
    }
}

/// Final archive bytes paired with the only provisional view that may enter
/// strong object and digest verification. The provisional view differs only
/// at the complete, disjoint set of decoded 32-byte digest sites.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
    objects: Vec<VerifiedNormalizedProvisionalScoopLirObjectV1>,
}

impl VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
    pub fn objects(&self) -> &[VerifiedNormalizedProvisionalScoopLirObjectV1] {
        &self.objects
    }

    pub fn candidates(&self) -> Vec<ScoopLirObjectCandidateV1<'_>> {
        self.objects
            .iter()
            .map(|object| ScoopLirObjectCandidateV1::new(object.member, &object.provisional_bytes))
            .collect()
    }
}

pub fn normalize_final_scoop_lir_objects_v1(
    member_plan: &PlannedLinkObjectMemberSetV1,
    final_objects: &[ScoopLirObjectCandidateV1<'_>],
    sites: &[ProvisionalDigestPatchSiteV1],
) -> Result<VerifiedNormalizedProvisionalScoopLirObjectSetV1, FinalObjectNormalizationError> {
    let expected = member_plan
        .scoop_lir_members()
        .iter()
        .map(|member| member.member_id())
        .collect::<Vec<_>>();
    let actual = final_objects
        .iter()
        .map(|object| object.member())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(FinalObjectNormalizationError::MemberSetMismatch);
    }

    let indexes = actual
        .iter()
        .enumerate()
        .map(|(index, member)| (*member, index))
        .collect::<BTreeMap<_, _>>();
    let mut ranges = BTreeMap::<SlibMemberId, BTreeSet<(usize, usize)>>::new();
    for site in sites {
        if site.width_bytes() != DIGEST_WIDTH as u8 {
            return Err(FinalObjectNormalizationError::PatchWidth {
                member: site.member(),
                actual: site.width_bytes(),
            });
        }
        let index = indexes.get(&site.member()).copied().ok_or(
            FinalObjectNormalizationError::UnknownPatchMember(site.member()),
        )?;
        let start = usize::try_from(site.checked_offset()).map_err(|_| {
            FinalObjectNormalizationError::PatchRange {
                member: site.member(),
                offset: site.checked_offset(),
            }
        })?;
        let end =
            start
                .checked_add(DIGEST_WIDTH)
                .ok_or(FinalObjectNormalizationError::PatchRange {
                    member: site.member(),
                    offset: site.checked_offset(),
                })?;
        if end > final_objects[index].bytes().len() {
            return Err(FinalObjectNormalizationError::PatchRange {
                member: site.member(),
                offset: site.checked_offset(),
            });
        }
        let member_ranges = ranges.entry(site.member()).or_default();
        if member_ranges
            .range(..(end, 0))
            .next_back()
            .is_some_and(|(_, existing_end)| *existing_end > start)
        {
            return Err(FinalObjectNormalizationError::OverlappingPatch {
                member: site.member(),
                offset: site.checked_offset(),
            });
        }
        member_ranges.insert((start, end));
    }

    let mut objects = final_objects
        .iter()
        .map(|object| VerifiedNormalizedProvisionalScoopLirObjectV1 {
            member: object.member(),
            final_bytes: object.bytes().to_vec(),
            provisional_bytes: object.bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    for (member, member_ranges) in ranges {
        let index = indexes[&member];
        for (start, end) in member_ranges {
            objects[index].provisional_bytes[start..end].fill(0);
        }
    }
    Ok(VerifiedNormalizedProvisionalScoopLirObjectSetV1 { objects })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinalObjectNormalizationError {
    MemberSetMismatch,
    UnknownPatchMember(SlibMemberId),
    PatchWidth { member: SlibMemberId, actual: u8 },
    PatchRange { member: SlibMemberId, offset: u64 },
    OverlappingPatch { member: SlibMemberId, offset: u64 },
}

impl fmt::Display for FinalObjectNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid final Scoop object normalization: {self:?}"
        )
    }
}

impl std::error::Error for FinalObjectNormalizationError {}
