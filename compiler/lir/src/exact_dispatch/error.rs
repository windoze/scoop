use scoop_identity::{
    PersistentDispatchSlotId, PersistentDispatchTableId, StrongCallableDefinitionOwner,
};
use scoop_wire::{HashError, WireError};

use super::ExactDispatchWireError;

#[derive(Debug)]
pub enum ExactDispatchError {
    CountOverflow,
    MissingFoundationTable(PersistentDispatchTableId),
    FoundationTableKey(PersistentDispatchTableId),
    SlotCount { expected: usize, actual: usize },
    Position { expected: u32, actual: u32 },
    DuplicateSlot(PersistentDispatchSlotId),
    AbiTarget(StrongCallableDefinitionOwner),
    AbiTargetProfile(StrongCallableDefinitionOwner),
    AbiDefinition(StrongCallableDefinitionOwner),
    AbiSignature(StrongCallableDefinitionOwner),
    ReceiverAdaptation(StrongCallableDefinitionOwner),
    ReceiverLayout(StrongCallableDefinitionOwner),
    UnexpectedReceiverLayout(StrongCallableDefinitionOwner),
    MissingPhysicalCallable(u32),
    PhysicalCallable(u32),
    DefinitionSubject(PersistentDispatchTableId),
    Definition(crate::StrongShapeDefinitionError),
    Hash(HashError),
    Resource(WireError),
}

impl From<crate::StrongShapeDefinitionError> for ExactDispatchError {
    fn from(error: crate::StrongShapeDefinitionError) -> Self {
        Self::Definition(error)
    }
}

impl From<HashError> for ExactDispatchError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}

impl From<WireError> for ExactDispatchError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ExactDispatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "exact dispatch replay failed: {self:?}")
    }
}

impl std::error::Error for ExactDispatchError {}

#[derive(Debug)]
pub enum ExactDispatchTableError {
    LayoutProvider,
    LayoutTarget,
    CountOverflow,
    Count {
        expected: usize,
        actual: usize,
    },
    Duplicate(PersistentDispatchTableId),
    Missing(PersistentDispatchTableId),
    Target(PersistentDispatchTableId),
    Provider(PersistentDispatchTableId),
    Definition(PersistentDispatchTableId),
    PhysicalDefinition(crate::StrongShapeDefinitionError),
    Record {
        index: usize,
        source: ExactDispatchWireError,
    },
    Resource(WireError),
}

impl From<crate::StrongShapeDefinitionError> for ExactDispatchTableError {
    fn from(error: crate::StrongShapeDefinitionError) -> Self {
        Self::PhysicalDefinition(error)
    }
}

impl From<WireError> for ExactDispatchTableError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ExactDispatchTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid exact dispatch table: {self:?}")
    }
}

impl std::error::Error for ExactDispatchTableError {}
