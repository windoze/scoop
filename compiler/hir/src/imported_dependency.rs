//! Request-local HIR handles for executable ordinary-dependency callables.

/// Export-HIR use of one callable committed through the ordinary-dependency
/// selection transaction.
///
/// The reference names the actual typed declaration. The complete output
/// owns its selected interface for subsequent stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedDependencyCallableUse {
    reference: crate::ImportedDependencyCallableRef,
    dispatch: ImportedDependencyDispatch,
}

/// The source-selected dispatch table and its provider-defined slot position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportedDependencyDispatch {
    Direct,
    Virtual {
        slot: u32,
    },
    Interface {
        interface: scoop_identity::PersistentTypeId,
        slot: u32,
    },
}

impl ImportedDependencyCallableUse {
    pub fn new(
        reference: crate::ImportedDependencyCallableRef,
        dispatch: ImportedDependencyDispatch,
    ) -> Self {
        Self {
            reference,
            dispatch,
        }
    }

    pub const fn reference(self) -> crate::ImportedDependencyCallableRef {
        self.reference
    }

    pub const fn dispatch(self) -> ImportedDependencyDispatch {
        self.dispatch
    }
}
