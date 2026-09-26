//! Request-local HIR handles for executable ordinary-dependency callables.

/// Export-HIR use of one callable committed through the ordinary-dependency
/// selection transaction.
///
/// The reference names the actual typed declaration. The complete output
/// owns its selected interface for subsequent stages.
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
