//! Effect-refined MIR uses of ordinary dependency callables.

use crate::{GcEffect, SelectedDependencyMirCallableRef};

/// One ordinary dependency callable admitted by a request-local selected MIR
/// bridge.
///
/// The selected reference and its GC protocol form one closed value. Call
/// sites therefore cannot attach an unrelated effect to a bare arena id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportedDependencyMirCallableUse {
    reference: SelectedDependencyMirCallableRef,
    effect: ImportedDependencyMirCallEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImportedDependencyMirCallEffect {
    Managed,
    NoGc,
}

impl ImportedDependencyMirCallableUse {
    pub(crate) const fn new(
        reference: SelectedDependencyMirCallableRef,
        gc_effect: GcEffect,
    ) -> Self {
        let effect = match gc_effect {
            GcEffect::Managed => ImportedDependencyMirCallEffect::Managed,
            GcEffect::NoGc => ImportedDependencyMirCallEffect::NoGc,
        };
        Self { reference, effect }
    }

    pub const fn reference(self) -> SelectedDependencyMirCallableRef {
        self.reference
    }

    pub const fn gc_effect(self) -> GcEffect {
        match self.effect {
            ImportedDependencyMirCallEffect::Managed => GcEffect::Managed,
            ImportedDependencyMirCallEffect::NoGc => GcEffect::NoGc,
        }
    }
}
