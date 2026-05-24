//! Native and extern declaration facts exported by HIR lowering.

use scoopc_source::SourceMapSpan;
use scoopc_types::{EffectRow, TypeId};

use crate::common::FactIdentity;

/// Facts for declarations that cross the native ABI boundary.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct NativeExternFacts {
    pub extern_functions: Vec<ExternFunctionFact>,
    pub native_callables: Vec<NativeCallableFact>,
    pub extern_globals: Vec<ExternGlobalFact>,
    pub extern_libraries: Vec<ExternLibraryFact>,
}

impl NativeExternFacts {
    /// Return whether no native or extern facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.extern_functions.is_empty()
            && self.native_callables.is_empty()
            && self.extern_globals.is_empty()
            && self.extern_libraries.is_empty()
    }
}

/// Metadata for an `@Extern` function declaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ExternFunctionFact {
    pub identity: FactIdentity,
    pub symbol: String,
    pub calling_convention: String,
    pub parameter_tys: Vec<TypeId>,
    pub return_ty: TypeId,
    pub effects: EffectRow,
}

/// Metadata for a body-bearing `@CallingConvention` function.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct NativeCallableFact {
    pub identity: FactIdentity,
    pub symbol: String,
    pub calling_convention: String,
    pub parameter_tys: Vec<TypeId>,
    pub return_ty: TypeId,
    pub effects: EffectRow,
}

/// Metadata for an `@Extern` global declaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ExternGlobalFact {
    pub identity: FactIdentity,
    pub symbol: String,
    pub ty: TypeId,
    pub mutable: bool,
}

/// Link metadata emitted by source-level extern library annotations.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternLibraryFact {
    pub name: String,
    pub source: Option<SourceMapSpan>,
}
