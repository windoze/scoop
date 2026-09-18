//! Request-local HIR handles for executable ordinary-dependency callables.

/// Export-HIR use of one callable committed through the ordinary-dependency
/// selection transaction.
///
/// The reference retains the transaction brand. The closed ordinary output
/// resolves it against its owned selected set before any later stage may
/// consume the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedDependencyCallableUse {
    reference: crate::ImportedDependencyCallableRef,
}

impl ImportedDependencyCallableUse {
    pub fn new(reference: crate::ImportedDependencyCallableRef) -> Self {
        Self { reference }
    }

    pub const fn reference(self) -> crate::ImportedDependencyCallableRef {
        self.reference
    }
}
