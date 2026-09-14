use std::fmt;

use scoop_identity::{
    DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentExactTypeId,
};

use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeRegistrationPatchFailureV1 {
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
pub enum TypeRegistrationDigestPlanFailureV1 {
    MissingRegistrationObjectDefinitionNode,
    RegistrationObjectDefinitionNodeIdentity,
    RegistrationObjectDefinitionDirectInputs,
    RegistrationObjectDefinitionPatchSet,
    MissingDescriptorObjectDefinitionNode,
    DescriptorObjectDefinitionNodeIdentity,
    MissingLayoutNode,
    LayoutNodeIdentity,
    MissingRegistrationNode,
    RegistrationNodeIdentity,
    RegistrationDirectInputs,
    RegistrationPatchSet,
    DescriptorDefinitionPatchMissing,
    LayoutPatchMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeRegistrationAtomFileRangeFailureV1 {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeRegistrationRelocationFailureV1 {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeDescriptorDiagnosticRelocationFailureV1 {
    Count,
    ContainingAtomRole,
    SectionRole,
    OffsetWithinAtom,
    Width,
    Form,
    EncodedValue,
    TargetKind,
    TargetAtom,
    TargetSection,
    TargetValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeRegistrationValidationError {
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
        exact_type: PersistentExactTypeId,
        kind: TypeRegistrationDigestPlanFailureV1,
    },
    MissingDefinitionAssignment {
        exact_type: PersistentExactTypeId,
        definition: ObjectDefinitionPlanId,
    },
    DefinitionAssignedToNonScoopMember {
        exact_type: PersistentExactTypeId,
        member: SlibMemberId,
    },
    MissingVerifiedDefinition {
        exact_type: PersistentExactTypeId,
        definition: ObjectDefinitionPlanId,
    },
    PrimaryAtomMismatch {
        exact_type: PersistentExactTypeId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingPrimaryAtom {
        exact_type: PersistentExactTypeId,
        atom: ObjectDefinitionAtomId,
    },
    DescriptorPrimaryAtomMismatch {
        exact_type: PersistentExactTypeId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    LayoutPrimaryAtomMismatch {
        exact_type: PersistentExactTypeId,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingDescriptorPrimarySymbol {
        exact_type: PersistentExactTypeId,
    },
    MissingLayoutPrimarySymbol {
        exact_type: PersistentExactTypeId,
    },
    InvalidPrimaryAtomFileRange {
        exact_type: PersistentExactTypeId,
        atom: ObjectDefinitionAtomId,
        kind: TypeRegistrationAtomFileRangeFailureV1,
    },
    PrimaryAtomSectionMismatch {
        exact_type: PersistentExactTypeId,
    },
    PrimaryAtomSizeMismatch {
        exact_type: PersistentExactTypeId,
        actual: u64,
    },
    MissingDescriptorPrimaryAtom {
        exact_type: PersistentExactTypeId,
        atom: ObjectDefinitionAtomId,
    },
    InvalidDescriptorPrimaryAtomFileRange {
        exact_type: PersistentExactTypeId,
        atom: ObjectDefinitionAtomId,
        kind: TypeRegistrationAtomFileRangeFailureV1,
    },
    DescriptorPrimaryAtomSectionMismatch {
        exact_type: PersistentExactTypeId,
    },
    DescriptorPrimaryAtomSizeMismatch {
        exact_type: PersistentExactTypeId,
        actual: u64,
    },
    MissingDescriptorDiagnosticAtom {
        exact_type: PersistentExactTypeId,
        atom: ObjectDefinitionAtomId,
    },
    InvalidDescriptorDiagnosticAtomFileRange {
        exact_type: PersistentExactTypeId,
        atom: ObjectDefinitionAtomId,
        kind: TypeRegistrationAtomFileRangeFailureV1,
    },
    DescriptorDiagnosticAtomSectionMismatch {
        exact_type: PersistentExactTypeId,
    },
    DescriptorDiagnosticAtomSizeMismatch {
        exact_type: PersistentExactTypeId,
        expected: u64,
        actual: u64,
    },
    DescriptorDiagnosticByteMismatch {
        exact_type: PersistentExactTypeId,
        offset_within_atom: u64,
        expected: u8,
        actual: u8,
    },
    DescriptorDiagnosticRelocationMismatch {
        exact_type: PersistentExactTypeId,
        kind: TypeDescriptorDiagnosticRelocationFailureV1,
    },
    DescriptorRelocationMismatch {
        exact_type: PersistentExactTypeId,
        kind: TypeRegistrationRelocationFailureV1,
    },
    MissingPatch {
        exact_type: PersistentExactTypeId,
        intent: DigestPatchIntentId,
    },
    PatchMismatch {
        exact_type: PersistentExactTypeId,
        intent: DigestPatchIntentId,
        kind: TypeRegistrationPatchFailureV1,
    },
    UnexpectedPatchInPrimaryAtom {
        exact_type: PersistentExactTypeId,
        intent: DigestPatchIntentId,
    },
    RecordRangeOverflow(PersistentExactTypeId),
    RecordByteMismatch {
        exact_type: PersistentExactTypeId,
        offset_within_atom: u16,
        expected: u8,
        actual: u8,
    },
}

impl fmt::Display for StrongTypeRegistrationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong type registration object set: {self:?}"
        )
    }
}

impl std::error::Error for StrongTypeRegistrationValidationError {}
