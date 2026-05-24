//! MIR-owned root inventories published for downstream stages.

use scoopc_span::Span;
use scoopc_types::TypeId;

use crate::common::{FactIdentity, MirBodyReference};

/// All direct-style MIR roots that downstream stages may need to query.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RootInventories {
    pub callable_bodies: Vec<MirRootFact>,
    pub initializers: Vec<MirRootFact>,
    pub initializer_dependencies: Vec<MirInitializerDependencyFact>,
    pub extern_globals: Vec<MirRootFact>,
    pub metadata_roots: Vec<MirRootFact>,
}

impl RootInventories {
    /// Return whether no root facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.callable_bodies.is_empty()
            && self.initializers.is_empty()
            && self.initializer_dependencies.is_empty()
            && self.extern_globals.is_empty()
            && self.metadata_roots.is_empty()
    }

    /// Return callable body root facts in stable FQN order.
    pub fn callable_body_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.callable_bodies.iter().map(|root| root.fqn.as_str())
    }

    /// Return initializer root facts in stable FQN order.
    pub fn initializer_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.initializers.iter().map(|root| root.fqn.as_str())
    }

    /// Return extern/global root facts in stable FQN order.
    pub fn extern_global_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.extern_globals.iter().map(|root| root.fqn.as_str())
    }

    /// Return metadata root facts in stable FQN order.
    pub fn metadata_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.metadata_roots.iter().map(|root| root.fqn.as_str())
    }

    /// Find a callable body root fact by FQN.
    pub fn callable_body(&self, fqn: &str) -> Option<&MirRootFact> {
        root_by_fqn(&self.callable_bodies, fqn)
    }

    /// Find an initializer root fact by FQN.
    pub fn initializer(&self, fqn: &str) -> Option<&MirRootFact> {
        root_by_fqn(&self.initializers, fqn)
    }

    /// Return initializer dependency identities owned by the given initializer root.
    pub fn initializer_dependencies_for<'a>(
        &'a self,
        fqn: &'a str,
    ) -> impl Iterator<Item = &'a MirInitializerDependencyFact> + 'a {
        self.initializer_dependencies
            .iter()
            .filter(move |dependency| dependency.owner_fqn == fqn)
    }

    /// Find an extern/global root fact by FQN.
    pub fn extern_global(&self, fqn: &str) -> Option<&MirRootFact> {
        root_by_fqn(&self.extern_globals, fqn)
    }

    /// Find a metadata root fact by FQN.
    pub fn metadata_root(&self, fqn: &str) -> Option<&MirRootFact> {
        root_by_fqn(&self.metadata_roots, fqn)
    }
}

fn root_by_fqn<'a>(roots: &'a [MirRootFact], fqn: &str) -> Option<&'a MirRootFact> {
    roots.iter().find(|root| root.fqn == fqn)
}

/// A single root inventory entry owned by the MIR stage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirRootFact {
    pub identity: FactIdentity,
    pub kind: MirRootKind,
    pub fqn: String,
    pub item: MirItemReference,
    pub ty: Option<TypeId>,
    pub body: Option<MirBodyReference>,
    pub source_path: Option<String>,
    pub source_cone_order: Option<u32>,
    pub span: Option<Span>,
    pub detail: MirRootDetail,
}

impl MirRootFact {
    /// Create a root fact without exposing MIR item or body node types.
    pub fn new(
        identity: FactIdentity,
        kind: MirRootKind,
        fqn: impl Into<String>,
        item: MirItemReference,
        detail: MirRootDetail,
    ) -> Self {
        Self {
            identity,
            kind,
            fqn: fqn.into(),
            item,
            ty: None,
            body: None,
            source_path: None,
            source_cone_order: None,
            span: None,
            detail,
        }
    }

    /// Attach the root's MIR type reference when one is available.
    pub fn with_ty(mut self, ty: Option<TypeId>) -> Self {
        self.ty = ty;
        self
    }

    /// Attach the root's MIR body reference when one is available.
    pub fn with_body(mut self, body: Option<MirBodyReference>) -> Self {
        self.body = body;
        self
    }

    /// Attach the root's stable source path when one is available.
    pub fn with_source_path(mut self, source_path: Option<String>) -> Self {
        self.source_path = source_path;
        self
    }

    /// Attach the dependency-before-consumer source-cone topo order when known.
    pub fn with_source_cone_order(mut self, source_cone_order: Option<u32>) -> Self {
        self.source_cone_order = source_cone_order;
        self
    }

    /// Attach the root's local source span when one is available.
    pub fn with_span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }
}

/// Stable reference to an item inside the direct-style MIR file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MirItemReference {
    pub index: usize,
}

impl MirItemReference {
    /// Create a reference by stable MIR item order.
    pub const fn new(index: usize) -> Self {
        Self { index }
    }
}

/// A dependency edge between MIR-published initializer roots.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirInitializerDependencyFact {
    pub owner_fqn: String,
    pub target_fqn: String,
    pub kind: MirInitializerDependencyKind,
}

/// Stable dependency categories for initializer root ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirInitializerDependencyKind {
    TopLevelValue,
    ObjectSingleton,
}

impl MirInitializerDependencyKind {
    /// Return a stable dump label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::TopLevelValue => "top_level_value",
            Self::ObjectSingleton => "object_singleton",
        }
    }
}

/// Stable categories for MIR root inventories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirRootKind {
    CallableBody,
    Initializer,
    ExternGlobal,
    Metadata,
}

impl MirRootKind {
    /// Return a stable label for dumps and fact identity construction.
    pub const fn label(self) -> &'static str {
        match self {
            Self::CallableBody => "callable_body",
            Self::Initializer => "initializer",
            Self::ExternGlobal => "extern_global",
            Self::Metadata => "metadata",
        }
    }
}

/// Root-kind-specific data published without depending on MIR node types.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MirRootDetail {
    CallableBody,
    Initializer {
        kind: MirInitializerRootKind,
        has_initializer: bool,
        dependency_count: usize,
    },
    ExternGlobal {
        storage: MirGlobalStorageKind,
        mutable: bool,
        symbol: String,
        initializer_absent: bool,
        unsafe_required: bool,
    },
    Metadata {
        kind: MirMetadataRootKind,
    },
}

/// Stable categories for top-level initializer roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirInitializerRootKind {
    RuntimeImmutableVal,
    RuntimeMutableGlobalVar,
    RuntimeMutableThreadLocalVar,
    ObjectSingleton,
}

impl MirInitializerRootKind {
    /// Return a stable dump label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeImmutableVal => "runtime_immutable_val",
            Self::RuntimeMutableGlobalVar => "runtime_mutable_global_var",
            Self::RuntimeMutableThreadLocalVar => "runtime_mutable_thread_local_var",
            Self::ObjectSingleton => "object_singleton",
        }
    }
}

/// Stable storage categories for MIR-published global roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirGlobalStorageKind {
    Global,
    ThreadLocal,
}

impl MirGlobalStorageKind {
    /// Return a stable dump label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::ThreadLocal => "thread_local",
        }
    }
}

/// Stable categories for MIR declaration metadata roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirMetadataRootKind {
    TypeAlias,
    Nominal,
    Object,
    ExtensionProperty,
}

impl MirMetadataRootKind {
    /// Return a stable dump label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::TypeAlias => "type_alias",
            Self::Nominal => "nominal",
            Self::Object => "object",
            Self::ExtensionProperty => "extension_property",
        }
    }
}
