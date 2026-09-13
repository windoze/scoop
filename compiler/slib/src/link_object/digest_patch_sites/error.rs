use std::fmt;

use scoop_identity::{DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId};
use scoop_lir::{DefinitionAtomResolutionError, DigestPlanError};

use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomFileRangeFailure {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DigestPatchSiteValidationError {
    ProducerMismatch {
        object: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    DigestPlan(DigestPlanError),
    DuplicateObjectMember(SlibMemberId),
    NonCanonicalObjectMemberOrder {
        index: usize,
    },
    UnexpectedObjectMember(SlibMemberId),
    MissingObjectMember(SlibMemberId),
    MissingVerifiedObjectMember(SlibMemberId),
    ObjectBytesMismatch(SlibMemberId),
    InvalidPatchTarget {
        intent: DigestPatchIntentId,
        kind: DefinitionAtomResolutionError,
    },
    MissingTargetMember {
        intent: DigestPatchIntentId,
        definition: ObjectDefinitionPlanId,
    },
    NonScoopTargetMember {
        intent: DigestPatchIntentId,
        member: SlibMemberId,
    },
    DuplicateExpectedIntent(DigestPatchIntentId),
    DuplicatePatchSite(DigestPatchIntentId),
    NonCanonicalPatchSiteOrder {
        index: usize,
    },
    UnexpectedPatchIntent(DigestPatchIntentId),
    MissingPatchIntent(DigestPatchIntentId),
    PatchMemberMismatch {
        intent: DigestPatchIntentId,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    PatchWidthMismatch {
        intent: DigestPatchIntentId,
        actual: u8,
    },
    MissingVerifiedDefinition {
        intent: DigestPatchIntentId,
        definition: ObjectDefinitionPlanId,
    },
    MissingVerifiedAtom {
        intent: DigestPatchIntentId,
        atom: ObjectDefinitionAtomId,
    },
    InvalidAtomFileRange {
        intent: DigestPatchIntentId,
        atom: ObjectDefinitionAtomId,
        kind: AtomFileRangeFailure,
    },
    PatchOffsetOverflow(DigestPatchIntentId),
    PatchOutsideAtom {
        intent: DigestPatchIntentId,
        atom: ObjectDefinitionAtomId,
        atom_start: u64,
        atom_end: u64,
        patch_start: u64,
        patch_end: u64,
    },
    NonZeroProvisionalSlot {
        intent: DigestPatchIntentId,
        byte_offset: u64,
    },
    RelocationOverlapsPatch {
        intent: DigestPatchIntentId,
        relocation_offset: u64,
    },
    OverlappingPatchSites {
        first: DigestPatchIntentId,
        second: DigestPatchIntentId,
    },
}

impl fmt::Display for DigestPatchSiteValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Scoop LIR digest patch-site set: {self:?}"
        )
    }
}

impl std::error::Error for DigestPatchSiteValidationError {}
