//! External Scoop callable uses and their committed selection references.

use crate::{GcEffect, SelectedExternalMirCallableRef};

/// A selected external implementation and its inseparable caller GC effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalCallableUse {
    reference: SelectedExternalMirCallableRef,
    gc_effect: GcEffect,
}

impl ExternalCallableUse {
    pub(crate) const fn new(
        reference: SelectedExternalMirCallableRef,
        gc_effect: GcEffect,
    ) -> Self {
        Self {
            reference,
            gc_effect,
        }
    }
    pub const fn reference(self) -> SelectedExternalMirCallableRef {
        self.reference
    }
    pub const fn gc_effect(self) -> GcEffect {
        self.gc_effect
    }
}
