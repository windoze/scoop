use std::collections::BTreeMap;

use scoop_identity::{PersistentExactTypeId, SourceDeclarationKey};

use crate::{DeclarationAccessSourceV1, SourceNominalId};

mod edges;
mod graph;
mod interface;
mod objects;
mod queries;
mod source;

pub use edges::*;
pub use graph::{InheritanceGraphError, NominalInheritanceSemanticAuthority};
#[cfg(test)]
pub(super) use interface::tests::support as interface_test_support;
pub use interface::*;
pub use objects::CheckedObjectInheritanceRelationV1;
pub use queries::InheritanceQueryError;

/// Proves nominal identity, lexical source, edge kind, and acyclicity only.
/// Slot obligations, representation equality, and selected access are checked
/// separately before the enclosing section can become an external interface.
#[derive(Debug)]
pub struct CheckedNominalInheritanceGraphV1<'a> {
    nodes: BTreeMap<PersistentExactTypeId, CheckedInheritanceNodeV1<'a>>,
    sources: BTreeMap<SourceNominalId, CheckedInheritanceSourceV1<'a>>,
    object_backings: BTreeMap<PersistentExactTypeId, CheckedObjectInheritanceRelationV1>,
}

#[derive(Clone, Copy, Debug)]
pub struct CheckedInheritanceNodeV1<'a> {
    edges: &'a NominalInheritanceEdgesV1,
    source: SourceNominalId,
}
impl CheckedInheritanceNodeV1<'_> {
    pub const fn edges(&self) -> &NominalInheritanceEdgesV1 {
        self.edges
    }
    pub const fn source(&self) -> SourceNominalId {
        self.source
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CheckedInheritanceSourceV1<'a> {
    pub key: &'a SourceDeclarationKey,
    pub access: &'a DeclarationAccessSourceV1,
}

impl CheckedNominalInheritanceGraphV1<'_> {
    pub fn get(&self, exact: PersistentExactTypeId) -> Option<CheckedInheritanceNodeV1<'_>> {
        self.nodes.get(&exact).copied()
    }

    pub(super) fn source(&self, source: SourceNominalId) -> Option<CheckedInheritanceSourceV1<'_>> {
        self.sources.get(&source).copied()
    }
}

#[cfg(test)]
pub(in crate::cross_cone_type_semantics) mod tests;

#[cfg(test)]
pub(in crate::cross_cone_type_semantics) use interface::tests::support::fixture as inheritance_interface_fixture;
