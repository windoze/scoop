//! MIR-owned root inventories published for downstream stages.

use scoopc_types::TypeId;

use crate::common::{FactIdentity, MirBodyReference};

/// All direct-style MIR roots that downstream stages may need to query.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RootInventories {
    pub callable_bodies: Vec<MirRootFact>,
    pub initializers: Vec<MirRootFact>,
    pub extern_globals: Vec<MirRootFact>,
    pub metadata_roots: Vec<MirRootFact>,
}

impl RootInventories {
    /// Return whether no root facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.callable_bodies.is_empty()
            && self.initializers.is_empty()
            && self.extern_globals.is_empty()
            && self.metadata_roots.is_empty()
    }
}

/// A single root inventory entry owned by the MIR stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirRootFact {
    pub identity: FactIdentity,
    pub kind: MirRootKind,
    pub fqn: String,
    pub ty: Option<TypeId>,
    pub body: Option<MirBodyReference>,
}

impl MirRootFact {
    /// Create a root fact without exposing MIR item or body node types.
    pub fn new(
        identity: FactIdentity,
        kind: MirRootKind,
        fqn: impl Into<String>,
        ty: Option<TypeId>,
        body: Option<MirBodyReference>,
    ) -> Self {
        Self {
            identity,
            kind,
            fqn: fqn.into(),
            ty,
            body,
        }
    }
}

/// Stable categories for MIR root inventories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirRootKind {
    CallableBody,
    Initializer,
    ExternGlobal,
    Metadata,
}
