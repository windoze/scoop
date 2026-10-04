//! Errors from complete Scoop LIR stackmap verification.

use std::fmt;

use scoop_identity::{
    ConeIdentity, DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentCallableBodyId, PersistentSafepointSiteId,
};

use super::DarwinAarch64StackmapMachineCodeError;
use crate::SlibMemberId;
use crate::link_object::{ObjectStackmapSectionError, StackmapNormalizationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopLirStackmapValidationError {
    ProducerMismatch {
        object: ConeIdentity,
        semantic: ConeIdentity,
    },
    DuplicateObjectMember(SlibMemberId),
    NonCanonicalObjectOrder {
        index: usize,
    },
    UnexpectedObjectMember(SlibMemberId),
    MissingObjectMember(SlibMemberId),
    MissingVerifiedMember(SlibMemberId),
    MissingOwnerDefinitionAssignment {
        site: PersistentSafepointSiteId,
        owner: PersistentCallableBodyId,
    },
    OwnerAssignedToNonScoopMember {
        site: PersistentSafepointSiteId,
        member: SlibMemberId,
    },
    PhysicalSection {
        member: SlibMemberId,
        source: ObjectStackmapSectionError,
    },
    MissingStackmapSection(SlibMemberId),
    UnexpectedStackmapSection(SlibMemberId),
    NonStackmapAtomInSection {
        member: SlibMemberId,
        atom: ObjectDefinitionAtomId,
        actual: DefinitionAtomRole,
    },
    MemberRecordCoverage {
        member: SlibMemberId,
        expected: usize,
        actual: usize,
    },
    MemberFunctionCoverage {
        member: SlibMemberId,
        expected: usize,
        actual: usize,
    },
    UnplannedFunctionTarget {
        member: SlibMemberId,
        table_index: u32,
    },
    NonCallableFunctionTarget {
        member: SlibMemberId,
        table_index: u32,
    },
    UnexpectedFunctionOwner {
        member: SlibMemberId,
        owner: PersistentCallableBodyId,
    },
    DuplicateFunctionOwner {
        member: SlibMemberId,
        owner: PersistentCallableBodyId,
    },
    FunctionRecordCoverage {
        member: SlibMemberId,
        owner: PersistentCallableBodyId,
        expected: usize,
        actual: usize,
    },
    UnexpectedSafepointId {
        member: SlibMemberId,
        owner: PersistentCallableBodyId,
        safepoint_id: u64,
    },
    Normalization {
        member: SlibMemberId,
        safepoint_id: u64,
        source: StackmapNormalizationError,
    },
    DuplicateSite(PersistentSafepointSiteId),
    MissingVerifiedCallableDefinition {
        member: SlibMemberId,
        definition: ObjectDefinitionPlanId,
    },
    MachineCode {
        member: SlibMemberId,
        owner: PersistentCallableBodyId,
        source: DarwinAarch64StackmapMachineCodeError,
    },
    MissingFunctionOwner {
        member: SlibMemberId,
        owner: PersistentCallableBodyId,
    },
    MissingSite {
        member: SlibMemberId,
        site: PersistentSafepointSiteId,
    },
    GlobalRecordCoverage {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for ScoopLirStackmapValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Scoop LIR stackmap set: {self:?}")
    }
}

impl std::error::Error for ScoopLirStackmapValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PhysicalSection { source, .. } => Some(source),
            Self::Normalization { source, .. } => Some(source),
            Self::MachineCode { source, .. } => Some(source),
            _ => None,
        }
    }
}
