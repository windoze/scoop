//! External Scoop callable uses and their committed selection references.

use crate::{
    Callee, ExternalCallableUseId, GcEffect, Module, SelectedExternalMirCallableRef, StatementKind,
};
use std::collections::HashSet;

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

pub(crate) fn referenced_external_callables(module: &Module) -> HashSet<ExternalCallableUseId> {
    let mut referenced = HashSet::new();
    for (_, function) in module.functions.iter() {
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                let StatementKind::Call(effect) = &statement.kind else {
                    continue;
                };
                let call = match effect {
                    crate::CallEffect::Unit(call) | crate::CallEffect::Value { call, .. } => call,
                };
                if let Callee::External(callable) = call.target.callee {
                    referenced.insert(callable);
                }
            }
        }
    }
    referenced
}
