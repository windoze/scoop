use std::fmt;

use scoop_identity::{
    DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentStaticStorageId,
};

use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageArtifactRoleV1 {
    Registration,
    Storage,
    InitialTemplate,
    InitialRelocationTable,
    Layout,
    ScanProgram,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageRelocationRoleV1 {
    StoragePointer,
    ScanProgramPointer,
    InitialTemplatePointer,
    InitialRelocationTablePointer,
    InitialStorageValue { index: u32 },
    InitialRelocationTarget { index: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageRegistrationPatchFailureV1 {
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
pub enum StaticStorageRegistrationDigestPlanFailureV1 {
    MissingRegistrationObjectDefinitionNode,
    RegistrationObjectDefinitionNodeIdentity,
    RegistrationObjectDefinitionDirectInputs,
    RegistrationObjectDefinitionPatchSet,
    MissingStorageObjectDefinitionNode,
    StorageObjectDefinitionNodeIdentity,
    StorageObjectDefinitionDirectInputs,
    StorageObjectDefinitionPatchSet,
    MissingLayoutNode,
    LayoutNodeIdentity,
    LayoutDirectInputs,
    LayoutPatch,
    MissingScanNode,
    ScanNodeIdentity,
    ScanDirectInputs,
    ScanPatch,
    MissingRegistrationNode,
    RegistrationNodeIdentity,
    RegistrationDirectInputs,
    RegistrationPatchSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageAtomFileRangeFailureV1 {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageRegistrationRelocationFailureV1 {
    Count,
    MissingOffset,
    ContainingAtom,
    ContainingAtomRole,
    SectionRole,
    Width,
    Form,
    EncodedValue,
    TargetSlot,
    TargetKind,
    TargetAtom,
    TargetSection,
    TargetDefinition,
    TargetMember,
    TargetOwner,
    TargetSymbol,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageRegistrationValidationError {
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
        storage: PersistentStaticStorageId,
        kind: StaticStorageRegistrationDigestPlanFailureV1,
    },
    MissingDefinitionAssignment {
        storage: PersistentStaticStorageId,
        definition: ObjectDefinitionPlanId,
    },
    DefinitionAssignedToNonScoopMember {
        storage: PersistentStaticStorageId,
        member: SlibMemberId,
    },
    MissingVerifiedDefinition {
        storage: PersistentStaticStorageId,
        definition: ObjectDefinitionPlanId,
    },
    PrimaryAtomMismatch {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingAtom {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
        atom: ObjectDefinitionAtomId,
    },
    InvalidAtomFileRange {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
        atom: ObjectDefinitionAtomId,
        kind: StaticStorageAtomFileRangeFailureV1,
    },
    AtomSectionMismatch {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
    },
    AtomSizeMismatch {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
        expected: u64,
        actual: u64,
    },
    AtomAlignmentMismatch {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
        required: u64,
        address: u64,
    },
    ArtifactByteMismatch {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
        offset_within_atom: u64,
        expected: u8,
        actual: u8,
    },
    MissingPrimarySymbol {
        storage: PersistentStaticStorageId,
        role: StaticStorageArtifactRoleV1,
    },
    RelocationMismatch {
        storage: PersistentStaticStorageId,
        role: StaticStorageRelocationRoleV1,
        kind: StaticStorageRegistrationRelocationFailureV1,
    },
    MissingPatch {
        storage: PersistentStaticStorageId,
        intent: DigestPatchIntentId,
    },
    PatchMismatch {
        storage: PersistentStaticStorageId,
        intent: DigestPatchIntentId,
        kind: StaticStorageRegistrationPatchFailureV1,
    },
    UnexpectedPatchInPrimaryAtom {
        storage: PersistentStaticStorageId,
        intent: DigestPatchIntentId,
    },
    SentinelTargetMismatch(StaticStorageArtifactRoleV1),
    SentinelTargetCollision,
    RecordRangeOverflow(PersistentStaticStorageId),
    RecordByteMismatch {
        storage: PersistentStaticStorageId,
        offset_within_atom: u16,
        expected: u8,
        actual: u8,
    },
}

impl fmt::Display for StrongStaticStorageRegistrationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong static-storage registration set: {self:?}"
        )
    }
}

impl std::error::Error for StrongStaticStorageRegistrationValidationError {}
