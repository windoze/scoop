//! LocalConcrete handles for executable ordinary-dependency callables.

/// LocalConcrete-HIR use of one ordinary-dependency callable. This wrapper
/// owns a distinct arena-id domain while preserving the exact HIR selection
/// reference for MIR projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedDependencyCallableUse {
    reference: crate::ImportedDependencyCallableRef,
    dispatch: crate::ImportedDependencyDispatch,
    effect: scoop_identity::Effect,
}

impl ImportedDependencyCallableUse {
    pub fn from_export(source: crate::ImportedDependencyCallableUse) -> Self {
        Self {
            reference: source.reference(),
            dispatch: source.dispatch(),
            effect: source.effect(),
        }
    }

    pub const fn effect(self) -> scoop_identity::Effect {
        self.effect
    }

    pub const fn reference(self) -> crate::ImportedDependencyCallableRef {
        self.reference
    }

    pub const fn dispatch(self) -> crate::ImportedDependencyDispatch {
        self.dispatch
    }
}
