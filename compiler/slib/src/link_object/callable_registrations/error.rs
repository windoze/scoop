use std::fmt;

use scoop_identity::{
    DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentCallableBodyId,
};

use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableRegistrationPatchFailureV1 {
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
pub enum CallableRegistrationDigestPlanFailureV1 {
    MissingRegistrationObjectDefinitionNode,
    RegistrationObjectDefinitionNodeIdentity,
    RegistrationObjectDefinitionDirectInputs,
    RegistrationObjectDefinitionPatchSet,
    MissingBodyObjectDefinitionNode,
    BodyObjectDefinitionNodeIdentity,
    MissingRegistrationNode,
    RegistrationNodeIdentity,
    RegistrationDirectInputs,
    RegistrationPatchSet,
    BodyDefinitionPatchMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableRegistrationAtomFileRangeFailureV1 {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableRegistrationRelocationFailureV1 {
    Count,
    SourceMember,
    ContainingAtom,
    ContainingAtomRole,
    SectionRole,
    OffsetWithinAtom,
    Width,
    Form,
    EncodedValue,
    TargetSlot,
    TargetDefinition,
    TargetMember,
    TargetOwner,
    TargetSymbol,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableRegistrationValidationError {
    ContextKeys {
        body: PersistentCallableBodyId,
        field: &'static str,
    },
    DigestPatchProducerMismatch,
    DuplicateObjectMember(SlibMemberId),
    NonCanonicalObjectOrder {
        index: usize,
    },
    UnexpectedObjectMember(SlibMemberId),
    MissingObjectMember(SlibMemberId),
    MissingVerifiedMember(SlibMemberId),
    ObjectBytesMismatch(SlibMemberId),
    DigestPlanMismatch {
        body: PersistentCallableBodyId,
        kind: CallableRegistrationDigestPlanFailureV1,
    },
    MissingDefinitionAssignment {
        body: PersistentCallableBodyId,
        definition: ObjectDefinitionPlanId,
    },
    DefinitionAssignedToNonScoopMember {
        body: PersistentCallableBodyId,
        member: SlibMemberId,
    },
    MissingVerifiedDefinition {
        body: PersistentCallableBodyId,
        definition: ObjectDefinitionPlanId,
    },
    MissingVerifiedBodyDefinition {
        body: PersistentCallableBodyId,
        definition: ObjectDefinitionPlanId,
    },
    PrimaryAtomMismatch {
        body: PersistentCallableBodyId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingPrimaryAtom {
        body: PersistentCallableBodyId,
        atom: ObjectDefinitionAtomId,
    },
    BodyPrimaryAtomMismatch {
        body: PersistentCallableBodyId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingBodyPrimarySymbol {
        body: PersistentCallableBodyId,
    },
    InvalidPrimaryAtomFileRange {
        body: PersistentCallableBodyId,
        atom: ObjectDefinitionAtomId,
        kind: CallableRegistrationAtomFileRangeFailureV1,
    },
    PrimaryAtomSectionMismatch {
        body: PersistentCallableBodyId,
    },
    PrimaryAtomSizeMismatch {
        body: PersistentCallableBodyId,
        actual: u64,
    },
    EntryRelocationMismatch {
        body: PersistentCallableBodyId,
        kind: CallableRegistrationRelocationFailureV1,
    },
    MissingPatch {
        body: PersistentCallableBodyId,
        intent: DigestPatchIntentId,
    },
    PatchMismatch {
        body: PersistentCallableBodyId,
        intent: DigestPatchIntentId,
        kind: CallableRegistrationPatchFailureV1,
    },
    UnexpectedPatchInPrimaryAtom {
        body: PersistentCallableBodyId,
        intent: DigestPatchIntentId,
    },
    RecordRangeOverflow(PersistentCallableBodyId),
    RecordByteMismatch {
        body: PersistentCallableBodyId,
        offset_within_atom: u16,
        expected: u8,
        actual: u8,
    },
}

impl fmt::Display for StrongCallableRegistrationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong callable registration object set: {self:?}"
        )
    }
}

impl std::error::Error for StrongCallableRegistrationValidationError {}
