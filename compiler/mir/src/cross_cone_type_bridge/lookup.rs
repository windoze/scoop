//! Borrowed constituent queries. These indexes do not grant import selection.

use super::*;
use scoop_identity::StrongCallableDefinitionOwner;

mod indexes;

pub use indexes::*;

mod sealed {
    pub trait Sealed {}
}

pub trait MirTypeBridgeTypeLookupV1: sealed::Sealed {
    fn get(&self, exact: PersistentExactTypeId) -> Option<&ParamFreeMirTypeExportV1>;
    fn record_count(&self) -> usize;
}
pub trait MirTypeBridgeCallableLookupV1: sealed::Sealed {
    fn get(&self, target: StrongCallableDefinitionOwner) -> Option<&ParamFreeMirCallableBindingV1>;
    fn record_count(&self) -> usize;
}
pub trait MirTypeBridgeSchemaLookupV1: sealed::Sealed {
    fn get(&self, owner: PersistentExactTypeId) -> Option<&ParamFreeMirDispatchSchemaV1>;
    fn record_count(&self) -> usize;
}

impl sealed::Sealed for CanonicalParamFreeMirTypeExportsV1 {}
impl MirTypeBridgeTypeLookupV1 for CanonicalParamFreeMirTypeExportsV1 {
    fn get(&self, exact: PersistentExactTypeId) -> Option<&ParamFreeMirTypeExportV1> {
        self.get(exact)
    }
    fn record_count(&self) -> usize {
        self.records().len()
    }
}
impl sealed::Sealed for CanonicalMirCallableBindingsV1 {}
impl MirTypeBridgeCallableLookupV1 for CanonicalMirCallableBindingsV1 {
    fn get(&self, target: StrongCallableDefinitionOwner) -> Option<&ParamFreeMirCallableBindingV1> {
        self.get(target)
    }
    fn record_count(&self) -> usize {
        self.entries().len()
    }
}
impl sealed::Sealed for CanonicalMirDispatchSchemasV1 {}
impl MirTypeBridgeSchemaLookupV1 for CanonicalMirDispatchSchemasV1 {
    fn get(&self, owner: PersistentExactTypeId) -> Option<&ParamFreeMirDispatchSchemaV1> {
        self.get(owner)
    }
    fn record_count(&self) -> usize {
        self.records().len()
    }
}

#[derive(Debug)]
pub enum MirTypeBridgeLookupError {
    Resource(WireError),
    RecordCountOverflow,
    DuplicateType {
        exact: PersistentExactTypeId,
    },
    DuplicateCallable {
        target: StrongCallableDefinitionOwner,
    },
    DuplicateSchema {
        owner: PersistentExactTypeId,
    },
}
impl From<WireError> for MirTypeBridgeLookupError {
    fn from(value: WireError) -> Self {
        Self::Resource(value)
    }
}
impl std::fmt::Display for MirTypeBridgeLookupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR type bridge lookup: {self:?}")
    }
}
impl std::error::Error for MirTypeBridgeLookupError {}

#[cfg(test)]
mod tests;
