//! Global root and initialization facts accepted by the HIR barrier.

use scoopc_ids::CanonicalTextKey;
use scoopc_source::SourceMapSpan;
use scoopc_types::TypeId;

use crate::common::FactIdentity;

/// Facts describing top-level roots and source-level initializer metadata.
#[derive(Debug, Clone, Default)]
pub struct GlobalRootFacts {
    pub roots: Vec<GlobalRootFact>,
    pub object_initializers: Vec<InitializerFact>,
    pub class_initializers: Vec<InitializerFact>,
}

impl GlobalRootFacts {
    /// Return whether no global root or initializer facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
            && self.object_initializers.is_empty()
            && self.class_initializers.is_empty()
    }
}

/// Source-level global root family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalRootKind {
    TopLevelVal,
    TopLevelVar,
    ObjectSingleton,
}

/// Storage policy required for mutable top-level globals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalStoragePolicy {
    Global,
    ThreadLocal,
}

/// HIR fact for one legal, monomorphic global root.
#[derive(Debug, Clone)]
pub struct GlobalRootFact {
    pub identity: FactIdentity,
    pub kind: GlobalRootKind,
    pub ty: TypeId,
    pub storage: Option<GlobalStoragePolicy>,
    pub initializer: Option<SourceMapSpan>,
    pub monomorphic: bool,
}

/// HIR-owned initializer contract for object/class setup.
#[derive(Debug, Clone)]
pub struct InitializerFact {
    pub identity: FactIdentity,
    pub initialized_root: CanonicalTextKey,
    pub fields: Vec<InitializerFieldFact>,
}

/// One initialized field or property and its type.
#[derive(Debug, Clone)]
pub struct InitializerFieldFact {
    pub name: String,
    pub ty: TypeId,
    pub source: Option<SourceMapSpan>,
}
