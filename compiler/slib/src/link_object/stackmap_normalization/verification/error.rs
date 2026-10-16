//! Errors reading the stackmap data used by GC metadata and fingerprints.

use crate::SlibMemberId;
use crate::link_object::{ObjectStackmapSectionError, StackmapNormalizationError};
use scoop_identity::PersistentCallableBodyId;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopStackmapReadError {
    MissingMember(SlibMemberId),
    PhysicalSection {
        member: SlibMemberId,
        source: ObjectStackmapSectionError,
    },
    UnplannedFunctionTarget {
        member: SlibMemberId,
        table_index: u32,
    },
    NonCallableFunctionTarget {
        member: SlibMemberId,
        table_index: u32,
    },
    UnexpectedSafepointId {
        member: SlibMemberId,
        owner: PersistentCallableBodyId,
        safepoint_id: u64,
    },
    ReturnAddressOverflow {
        member: SlibMemberId,
        safepoint_id: u64,
    },
    Normalization {
        member: SlibMemberId,
        safepoint_id: u64,
        source: StackmapNormalizationError,
    },
}

impl fmt::Display for ScoopStackmapReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot read Scoop LIR stackmaps: {self:?}")
    }
}
impl std::error::Error for ScoopStackmapReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PhysicalSection { source, .. } => Some(source),
            Self::Normalization { source, .. } => Some(source),
            _ => None,
        }
    }
}
