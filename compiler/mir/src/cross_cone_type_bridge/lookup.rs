//! Borrowed constituent queries. These indexes do not grant import selection.

use super::*;

mod callables;
mod declarations;
mod indexes;

pub use callables::*;
pub use declarations::{
    dispatch_declaration_receiver, dispatch_declaration_target, dispatch_exact_declaration_target,
};
pub use indexes::*;

mod sealed {
    pub trait Sealed {}
}

pub trait MirTypeBridgeTypeLookupV1: sealed::Sealed {
    fn get(&self, exact: PersistentExactTypeId) -> Option<&ParamFreeMirTypeExportV1>;
    fn record_count(&self) -> usize;
    fn gc_kind(&self, exact: PersistentExactTypeId) -> Option<MirGcKindV1> {
        self.get(exact).map(|record| record.facts().gc())
    }
    fn exact_gc_kind(
        &self,
        identities: &ValidatedIdentityGraph,
        exact: PersistentExactTypeId,
    ) -> Result<MirGcKindV1, MirTypeBridgeError> {
        let key = identities.canonical_key::<_, ExactTypeKey>(exact)?;
        match key.as_ref() {
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => self
                .gc_kind(exact)
                .ok_or(MirTypeBridgeError::MissingType { exact }),
            ExactTypeKey::Tuple(elements) => {
                let mut gc = MirGcKindV1::GcFree;
                for element in elements.as_slice() {
                    if self.exact_gc_kind(identities, *element)?
                        == MirGcKindV1::ContainsManagedReferences
                    {
                        gc = MirGcKindV1::ContainsManagedReferences;
                    }
                }
                Ok(gc)
            }
            ExactTypeKey::Function { .. } => Ok(MirGcKindV1::ContainsManagedReferences),
            ExactTypeKey::RawPointer(_) | ExactTypeKey::NativeFunctionPointer { .. } => {
                Ok(MirGcKindV1::GcFree)
            }
        }
    }
}
pub trait MirTypeBridgeCallableLookupV1: sealed::Sealed {
    fn get(&self, target: CallableDefinitionOwner) -> Option<MirCallableRecordRefV1<'_>>;
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
    fn get(&self, target: CallableDefinitionOwner) -> Option<MirCallableRecordRefV1<'_>> {
        self.get(target).map(MirCallableRecordRefV1::Lowered)
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
    DuplicateType { exact: PersistentExactTypeId },
    DuplicateCallable { target: CallableDefinitionOwner },
    DuplicateSchema { owner: PersistentExactTypeId },
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
