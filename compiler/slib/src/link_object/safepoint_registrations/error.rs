use std::fmt;

use scoop_identity::{
    DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentSafepointSiteId,
};

use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointRegistrationSemanticFieldV1 {
    SafepointId,
    OwnerCallable,
    SiteRole,
    RootPairCount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointRegistrationPatchFailureV1 {
    Source,
    SemanticFieldRole,
    Member,
    Definition,
    Atom,
    AtomRole,
    SectionRole,
    OffsetWithinAtom,
    CheckedOffset,
    Width,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointRegistrationDigestPlanFailureV1 {
    MissingObjectDefinitionNode,
    MissingStackmapNode,
    MissingRegistrationNode,
    StackmapNodeIdentity,
    RegistrationNodeIdentity,
    RegistrationDirectInputs,
    RegistrationPatchSet,
    StackmapPatchSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointRegistrationAtomFileRangeFailureV1 {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongSafepointRegistrationValidationError {
    StackmapProducerMismatch,
    DigestPatchProducerMismatch,
    ObjectProofMismatch,
    DuplicateObjectMember(SlibMemberId),
    NonCanonicalObjectOrder {
        index: usize,
    },
    UnexpectedObjectMember(SlibMemberId),
    MissingObjectMember(SlibMemberId),
    MissingVerifiedMember(SlibMemberId),
    ObjectBytesMismatch(SlibMemberId),
    SiteCoverageMismatch {
        expected: Vec<PersistentSafepointSiteId>,
        actual: Vec<PersistentSafepointSiteId>,
    },
    StackmapSemanticMismatch {
        site: PersistentSafepointSiteId,
        field: SafepointRegistrationSemanticFieldV1,
    },
    DigestPlanMismatch {
        site: PersistentSafepointSiteId,
        kind: SafepointRegistrationDigestPlanFailureV1,
    },
    MissingDefinitionAssignment {
        site: PersistentSafepointSiteId,
        definition: ObjectDefinitionPlanId,
    },
    DefinitionAssignedToNonScoopMember {
        site: PersistentSafepointSiteId,
        member: SlibMemberId,
    },
    MissingVerifiedDefinition {
        site: PersistentSafepointSiteId,
        definition: ObjectDefinitionPlanId,
    },
    PrimaryAtomMismatch {
        site: PersistentSafepointSiteId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingPrimaryAtom {
        site: PersistentSafepointSiteId,
        atom: ObjectDefinitionAtomId,
    },
    InvalidPrimaryAtomFileRange {
        site: PersistentSafepointSiteId,
        atom: ObjectDefinitionAtomId,
        kind: SafepointRegistrationAtomFileRangeFailureV1,
    },
    PrimaryAtomSectionMismatch {
        site: PersistentSafepointSiteId,
    },
    PrimaryAtomSizeMismatch {
        site: PersistentSafepointSiteId,
        actual: u64,
    },
    UnexpectedRelocation {
        site: PersistentSafepointSiteId,
        offset_within_atom: u64,
    },
    MissingPatch {
        site: PersistentSafepointSiteId,
        intent: DigestPatchIntentId,
    },
    PatchMismatch {
        site: PersistentSafepointSiteId,
        intent: DigestPatchIntentId,
        kind: SafepointRegistrationPatchFailureV1,
    },
    UnexpectedPatchInPrimaryAtom {
        site: PersistentSafepointSiteId,
        intent: DigestPatchIntentId,
    },
    RecordRangeOverflow(PersistentSafepointSiteId),
    RecordByteMismatch {
        site: PersistentSafepointSiteId,
        offset_within_atom: u16,
        expected: u8,
        actual: u8,
    },
}

impl fmt::Display for StrongSafepointRegistrationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong safepoint registration object set: {self:?}"
        )
    }
}

impl std::error::Error for StrongSafepointRegistrationValidationError {}
