use std::fmt;

use scoop_identity::{
    DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentImmortalObjectId,
};

use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectRegistrationPatchFailureV1 {
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
pub enum ImmortalObjectRegistrationDigestPlanFailureV1 {
    MissingRegistrationObjectDefinitionNode,
    RegistrationObjectDefinitionNodeIdentity,
    RegistrationObjectDefinitionDirectInputs,
    RegistrationObjectDefinitionPatchSet,
    MissingImmortalObjectDefinitionNode,
    ImmortalObjectDefinitionNodeIdentity,
    ImmortalObjectDefinitionDirectInputs,
    ImmortalObjectDefinitionPatchSet,
    MissingRegistrationNode,
    RegistrationNodeIdentity,
    RegistrationDirectInputs,
    RegistrationPatchSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectRegistrationAtomFileRangeFailureV1 {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectRegistrationRelocationFailureV1 {
    Count,
    MissingOffset,
    ContainingAtom,
    ContainingAtomRole,
    SectionRole,
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
pub enum StrongImmortalObjectRegistrationValidationError {
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
        object: PersistentImmortalObjectId,
        kind: ImmortalObjectRegistrationDigestPlanFailureV1,
    },
    MissingDefinitionAssignment {
        object: PersistentImmortalObjectId,
        definition: ObjectDefinitionPlanId,
    },
    DefinitionAssignedToNonScoopMember {
        object: PersistentImmortalObjectId,
        member: SlibMemberId,
    },
    MissingVerifiedDefinition {
        object: PersistentImmortalObjectId,
        definition: ObjectDefinitionPlanId,
    },
    PrimaryAtomMismatch {
        object: PersistentImmortalObjectId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingPrimaryAtom {
        object: PersistentImmortalObjectId,
        atom: ObjectDefinitionAtomId,
    },
    ImmortalObjectPrimaryAtomMismatch {
        object: PersistentImmortalObjectId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    TypeRegistrationPrimaryAtomMismatch {
        object: PersistentImmortalObjectId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingImmortalObjectPrimarySymbol(PersistentImmortalObjectId),
    MissingTypeRegistrationPrimarySymbol(PersistentImmortalObjectId),
    InvalidPrimaryAtomFileRange {
        object: PersistentImmortalObjectId,
        atom: ObjectDefinitionAtomId,
        kind: ImmortalObjectRegistrationAtomFileRangeFailureV1,
    },
    PrimaryAtomSectionMismatch(PersistentImmortalObjectId),
    PrimaryAtomSizeMismatch {
        object: PersistentImmortalObjectId,
        actual: u64,
    },
    ObjectRelocationMismatch {
        object: PersistentImmortalObjectId,
        kind: ImmortalObjectRegistrationRelocationFailureV1,
    },
    TypeRegistrationRelocationMismatch {
        object: PersistentImmortalObjectId,
        kind: ImmortalObjectRegistrationRelocationFailureV1,
    },
    MissingPatch {
        object: PersistentImmortalObjectId,
        intent: DigestPatchIntentId,
    },
    PatchMismatch {
        object: PersistentImmortalObjectId,
        intent: DigestPatchIntentId,
        kind: ImmortalObjectRegistrationPatchFailureV1,
    },
    UnexpectedPatchInPrimaryAtom {
        object: PersistentImmortalObjectId,
        intent: DigestPatchIntentId,
    },
    RecordRangeOverflow(PersistentImmortalObjectId),
    RecordByteMismatch {
        object: PersistentImmortalObjectId,
        offset_within_atom: u16,
        expected: u8,
        actual: u8,
    },
}

impl fmt::Display for StrongImmortalObjectRegistrationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong immortal-object registration set: {self:?}"
        )
    }
}

impl std::error::Error for StrongImmortalObjectRegistrationValidationError {}
