use std::fmt;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, DigestPatchIntentId, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId,
};

use crate::SlibMemberId;
use crate::link_object::BuiltinObjectSectionRoleV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeImageAtomRoleV1 {
    Primary,
    CoordinateGroup,
    CoordinateName,
    CoordinateVersion,
    Dependencies,
    StaticStorages,
    ImmortalObjects,
    InitializationUnits,
    TypeRegistrations,
    Safepoints,
    Callables,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeImagePatchFailureV1 {
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
pub enum ConeImageRelocationFailureV1 {
    Count,
    MissingOffset,
    ContainingAtomRole,
    SectionRole,
    Width,
    Form,
    EncodedValue,
    TargetKind,
    TargetAtom,
    TargetSection,
    TargetValue,
    TargetSlot,
    TargetSymbol,
    TargetDefinition,
    TargetOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConeImageAtomFileRangeFailureV1 {
    MissingSection,
    NotFileBacked,
    InvalidRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConeImageValidationError {
    ProducerMismatch {
        image: ConeIdentity,
        patches: ConeIdentity,
    },
    DuplicateObjectMember(SlibMemberId),
    NonCanonicalObjectOrder {
        index: usize,
    },
    UnexpectedObjectMember(SlibMemberId),
    MissingObjectMember(SlibMemberId),
    MissingVerifiedMember(SlibMemberId),
    ObjectBytesMismatch(SlibMemberId),
    MissingDefinitionAssignment(ObjectDefinitionPlanId),
    DefinitionAssignedToNonScoopMember(SlibMemberId),
    ImageDefinitionSet {
        expected: ObjectDefinitionPlanId,
        actual: Vec<(SlibMemberId, ObjectDefinitionPlanId, ConeIdentity)>,
    },
    MissingVerifiedDefinition(ObjectDefinitionPlanId),
    PrimaryAtomMismatch {
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    AtomSetMismatch {
        expected: Vec<(ObjectDefinitionAtomId, DefinitionAtomRole)>,
        actual: Vec<(ObjectDefinitionAtomId, DefinitionAtomRole)>,
    },
    MissingAtom {
        role: ConeImageAtomRoleV1,
        atom: ObjectDefinitionAtomId,
    },
    InvalidAtomFileRange {
        role: ConeImageAtomRoleV1,
        atom: ObjectDefinitionAtomId,
        kind: ConeImageAtomFileRangeFailureV1,
    },
    AtomSectionMismatch {
        role: ConeImageAtomRoleV1,
        actual: BuiltinObjectSectionRoleV1,
    },
    AtomSizeMismatch {
        role: ConeImageAtomRoleV1,
        expected: u64,
        actual: u64,
    },
    AtomAlignmentMismatch {
        role: ConeImageAtomRoleV1,
        required: u64,
        address: u64,
    },
    AtomByteMismatch {
        role: ConeImageAtomRoleV1,
        offset_within_atom: u64,
        expected: u8,
        actual: u8,
    },
    RecordRangeOverflow(ConeImageAtomRoleV1),
    MissingPatch(DigestPatchIntentId),
    PatchMismatch {
        intent: DigestPatchIntentId,
        kind: ConeImagePatchFailureV1,
    },
    UnexpectedPatch {
        atom: ObjectDefinitionAtomId,
        intent: DigestPatchIntentId,
    },
    DigestNodeMismatch,
    RelocationMismatch {
        role: ConeImageAtomRoleV1,
        index: usize,
        kind: ConeImageRelocationFailureV1,
    },
}

impl fmt::Display for ConeImageValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid per-Cone runtime image object: {self:?}")
    }
}

impl std::error::Error for ConeImageValidationError {}
