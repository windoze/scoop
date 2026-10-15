use std::fmt;

use scoop_identity::{ConeIdentity, ObjectDefinitionAtomId};

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
    Resource(scoop_wire::WireError),
}

impl From<scoop_wire::WireError> for LayoutLinkClosureError {
    fn from(value: scoop_wire::WireError) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for LayoutLinkClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid cross-Cone layout Link closure: {self:?}")
    }
}
impl std::error::Error for LayoutLinkClosureError {}
