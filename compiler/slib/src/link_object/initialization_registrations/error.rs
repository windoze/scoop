use std::fmt;

use scoop_identity::{
    DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentInitializationUnitId,
};

use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationArtifactRoleV1 {
    Cell,
    CoordinatorDescriptor,
    Registration,
    DiagnosticBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationRelocationRoleV1 {
    CoordinatorDiagnostic,
    CoordinatorCell,
    CoordinatorStorage,
    CoordinatorFailureRoot,
    CoordinatorInitializer,
    CoordinatorEnsure,
    RegistrationDiagnostic,
    RegistrationCell,
    RegistrationStorage,
    RegistrationFailureRoot,
    RegistrationInitializer,
    RegistrationEnsure,
    RegistrationGateway,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationRegistrationPatchFailureV1 {
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
pub enum InitializationRegistrationDigestPlanFailureV1 {
    MissingRegistrationObjectDefinitionNode,
    RegistrationObjectDefinitionNodeIdentity,
    RegistrationObjectDefinitionDirectInputs,
    RegistrationObjectDefinitionPatchSet,
    MissingCellObjectDefinitionNode,
    CellObjectDefinitionNodeIdentity,
    CellObjectDefinitionDirectInputs,
    CellObjectDefinitionPatchSet,
    MissingDescriptorObjectDefinitionNode,
    DescriptorObjectDefinitionNodeIdentity,
    DescriptorObjectDefinitionDirectInputs,
    DescriptorObjectDefinitionPatchSet,
    MissingRegistrationNode,
    RegistrationNodeIdentity,
    RegistrationDirectInputs,
    RegistrationPatchSet,
    MissingGatewayObjectDefinitionNode,
    GatewayObjectDefinitionNodeIdentity,
    GatewayDefinitionPatchSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationAtomFileRangeFailureV1 {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationRelocationFailureV1 {
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
    TargetDefinition,
    TargetMember,
    TargetOwner,
    TargetSymbol,
    DiagnosticTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationRegistrationValidationError {
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
        unit: PersistentInitializationUnitId,
        kind: InitializationRegistrationDigestPlanFailureV1,
    },
    MissingDefinitionAssignment {
        unit: PersistentInitializationUnitId,
        definition: ObjectDefinitionPlanId,
    },
    DefinitionAssignedToNonScoopMember {
        unit: PersistentInitializationUnitId,
        member: SlibMemberId,
    },
    MissingVerifiedDefinition {
        unit: PersistentInitializationUnitId,
        definition: ObjectDefinitionPlanId,
    },
    PrimaryAtomMismatch {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    MissingPrimaryAtom {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
        atom: ObjectDefinitionAtomId,
    },
    MissingPrimarySymbol {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
    },
    MissingDiagnosticAtom {
        unit: PersistentInitializationUnitId,
        atom: ObjectDefinitionAtomId,
    },
    InvalidPrimaryAtomFileRange {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
        atom: ObjectDefinitionAtomId,
        kind: InitializationAtomFileRangeFailureV1,
    },
    PrimaryAtomSectionMismatch {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
    },
    PrimaryAtomSizeMismatch {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
        expected: u64,
        actual: u64,
    },
    PrimaryAtomAlignmentMismatch {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
        address: u64,
    },
    RelocationMismatch {
        unit: PersistentInitializationUnitId,
        role: InitializationRelocationRoleV1,
        kind: InitializationRelocationFailureV1,
    },
    MissingPatch {
        unit: PersistentInitializationUnitId,
        intent: DigestPatchIntentId,
    },
    PatchMismatch {
        unit: PersistentInitializationUnitId,
        intent: DigestPatchIntentId,
        kind: InitializationRegistrationPatchFailureV1,
    },
    UnexpectedPatchInPrimaryAtom {
        unit: PersistentInitializationUnitId,
        intent: DigestPatchIntentId,
    },
    RecordRangeOverflow(PersistentInitializationUnitId),
    RecordByteMismatch {
        unit: PersistentInitializationUnitId,
        role: InitializationArtifactRoleV1,
        offset_within_atom: u16,
        expected: u8,
        actual: u8,
    },
}

impl fmt::Display for StrongInitializationRegistrationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong initialization registration object set: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationRegistrationValidationError {}
