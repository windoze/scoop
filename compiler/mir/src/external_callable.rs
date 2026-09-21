//! External Scoop callable uses and their committed selection origins.

use std::collections::HashSet;

use crate::{
    Callee, ExternalCallableUseId, GcEffect, ImportedCoreMirCallableRef, MirMeta, Module,
    SelectedDependencyMirCallableRef, StatementKind,
};

/// A selected external implementation and its inseparable caller GC effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalCallableUse {
    selection: ExternalCallableSelection,
    gc_effect: GcEffect,
}

/// The selection record is distinct from the MIR use allocated from it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalCallableSelection {
    InitializationCycle(ImportedCoreMirCallableRef),
    Dependency(SelectedDependencyMirCallableRef),
}

impl ExternalCallableUse {
    pub(crate) const fn initialization_cycle(reference: ImportedCoreMirCallableRef) -> Self {
        Self {
            selection: ExternalCallableSelection::InitializationCycle(reference),
            gc_effect: GcEffect::Managed,
        }
    }

    pub(crate) const fn dependency(
        reference: SelectedDependencyMirCallableRef,
        gc_effect: GcEffect,
    ) -> Self {
        Self {
            selection: ExternalCallableSelection::Dependency(reference),
            gc_effect,
        }
    }

    pub const fn selection(self) -> ExternalCallableSelection {
        self.selection
    }

    pub const fn gc_effect(self) -> GcEffect {
        self.gc_effect
    }
}

impl MirMeta {
    pub(crate) fn initialization_callables(
        &self,
    ) -> impl Iterator<Item = (ExternalCallableUseId, ImportedCoreMirCallableRef)> + '_ {
        self.external_callables
            .iter()
            .filter_map(|(id, callable)| match callable.selection() {
                ExternalCallableSelection::InitializationCycle(reference) => Some((id, reference)),
                ExternalCallableSelection::Dependency(_) => None,
            })
    }

    pub(crate) fn dependency_callables(
        &self,
    ) -> impl Iterator<Item = (ExternalCallableUseId, SelectedDependencyMirCallableRef)> + '_ {
        self.external_callables
            .iter()
            .filter_map(|(id, callable)| match callable.selection() {
                ExternalCallableSelection::Dependency(reference) => Some((id, reference)),
                ExternalCallableSelection::InitializationCycle(_) => None,
            })
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
