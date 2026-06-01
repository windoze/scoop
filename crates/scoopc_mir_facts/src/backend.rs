//! Backend-facing MIR contracts published as facts instead of side-table-only data.

use scoopc_ids::StageArtifactKey;
use scoopc_span::Span;
use scoopc_types::TypeId;

use crate::common::FactIdentity;

/// Backend contracts carried by the MIR fact artifact.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirBackendFacts {
    pub source_signatures: Vec<SourceCallableSignatureFact>,
    pub enum_layouts: Vec<EnumLayoutContractFact>,
    pub class_inits: Vec<ClassInitContractFact>,
    pub vtables: Vec<VtableContractFact>,
    pub interfaces: Vec<InterfaceContractFact>,
    pub itables: Vec<ItableContractFact>,
    pub extern_funs: Vec<ExternFunContractFact>,
    pub native_callable_funs: Vec<NativeCallableFunContractFact>,
    pub global_inits: Vec<GlobalInitContractFact>,
}

impl MirBackendFacts {
    /// Return whether no backend facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.source_signatures.is_empty()
            && self.enum_layouts.is_empty()
            && self.class_inits.is_empty()
            && self.vtables.is_empty()
            && self.interfaces.is_empty()
            && self.itables.is_empty()
            && self.extern_funs.is_empty()
            && self.native_callable_funs.is_empty()
            && self.global_inits.is_empty()
    }
}

/// Source signature for a callable published by MIR materialization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceCallableSignatureFact {
    pub identity: FactIdentity,
    pub fqn: String,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
}

/// Enum layout summary needed by backend lowering.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnumLayoutContractFact {
    pub identity: FactIdentity,
    pub fqn: String,
    pub repr: String,
    pub variant_count: usize,
}

/// Class initialization/layout summary needed by backend lowering.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClassInitContractFact {
    pub identity: FactIdentity,
    pub key: String,
    pub fqn: String,
    pub source_path: String,
    pub super_class_fqn: Option<String>,
    pub field_count: usize,
    pub ctor_count: usize,
    pub step_count: usize,
}

/// Vtable layout summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VtableContractFact {
    pub identity: FactIdentity,
    pub class_fqn: String,
    pub slot_count: usize,
}

/// Interface slot layout summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InterfaceContractFact {
    pub identity: FactIdentity,
    pub interface_fqn: String,
    pub interface_id: u64,
    pub super_interfaces: Vec<String>,
    pub method_slot_count: usize,
}

/// Class itable summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ItableContractFact {
    pub identity: FactIdentity,
    pub class_fqn: String,
    pub entry_count: usize,
}

/// Extern callable contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternFunContractFact {
    pub identity: FactIdentity,
    pub fqn: String,
    pub symbol: String,
    pub abi: String,
    pub calling_convention: Option<String>,
    pub lib: Option<String>,
}

/// Native callable wrapper contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NativeCallableFunContractFact {
    pub identity: FactIdentity,
    pub fqn: String,
    pub symbol: String,
    pub calling_convention: String,
}

/// Top-level value/global initialization contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GlobalInitContractFact {
    pub identity: FactIdentity,
    pub fqn: String,
    pub kind: GlobalInitKind,
    pub ty: Option<TypeId>,
    pub source_path: Option<String>,
    pub span: Option<Span>,
    pub storage: Option<GlobalStorageKind>,
    pub has_initializer: bool,
    pub artifact: Option<StageArtifactKey>,
}

/// Stable global/init families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GlobalInitKind {
    RuntimeImmutableValue,
    RuntimeMutableVar,
    ObjectSingleton,
}

/// Stable global storage families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GlobalStorageKind {
    Global,
    ThreadLocal,
}
