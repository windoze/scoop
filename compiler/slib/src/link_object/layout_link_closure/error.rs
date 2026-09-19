use std::fmt;

use scoop_identity::{ConeIdentity, ObjectDefinitionAtomId};
use scoop_lir::ExternalStrongShapeSubjectV1;

use crate::{SlibMemberId, link_object::RelocationTargetSlotV1};

#[derive(Debug)]
pub enum LayoutLinkClosureError {
    ConsumerMismatch {
        objects: ConeIdentity,
        selection: ConeIdentity,
    },
    ImportIndexOverflow,
    DuplicateNormalizedSymbol {
        first: u32,
        second: u32,
    },
    OldPartition {
        import_index: u32,
    },
    NonExternalRemainder {
        member: SlibMemberId,
        atom: ObjectDefinitionAtomId,
    },
    DuplicateUse {
        member: SlibMemberId,
        atom: ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    UnusedImport {
        import_index: u32,
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    },
    ObjectProofMismatch,
    UseOutsideObjectSet {
        member: SlibMemberId,
    },
    RequirementsMismatch,
    ObjectCoverageMismatch,
    Resource(scoop_wire::WireError),
    SemanticImports(scoop_lir::ShapeLinkError),
    Hash(scoop_wire::HashError),
}

impl From<scoop_wire::WireError> for LayoutLinkClosureError {
    fn from(value: scoop_wire::WireError) -> Self {
        Self::Resource(value)
    }
}
impl From<scoop_lir::ShapeLinkError> for LayoutLinkClosureError {
    fn from(value: scoop_lir::ShapeLinkError) -> Self {
        Self::SemanticImports(value)
    }
}
impl From<scoop_wire::HashError> for LayoutLinkClosureError {
    fn from(value: scoop_wire::HashError) -> Self {
        Self::Hash(value)
    }
}
impl fmt::Display for LayoutLinkClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid cross-Cone layout Link closure: {self:?}")
    }
}
impl std::error::Error for LayoutLinkClosureError {}
