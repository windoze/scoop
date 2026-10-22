//! Expand known intrinsic entries independently of ordinary inline policy.

use std::collections::HashSet;

use scoop_hir::concrete as hir;

use super::{graph::index, inline, mir};
use crate::local_values::LocalValueRegistry;

#[derive(Clone, Default)]
pub(super) struct Entries {
    pub local: Vec<bool>,
    pub external: HashSet<mir::ExternalCallableUseId>,
}

impl Entries {
    pub fn contains(&self, module: &mir::Module, callee: mir::Callee) -> bool {
        match callee {
            mir::Callee::User(id) => self.local.get(index(id)).copied().unwrap_or(false),
            mir::Callee::Monomorphized(id) => self
                .local
                .get(index(module.meta.instances[id].function))
                .copied()
                .unwrap_or(false),
            mir::Callee::External(id) => self.external.contains(&id),
            _ => false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.external.is_empty() && !self.local.iter().any(|required| *required)
    }
}

pub(super) fn entries(
    module: &mir::Module,
    source: &hir::Module,
    external: HashSet<mir::ExternalCallableUseId>,
) -> Entries {
    let mut identities = source
        .functions
        .iter()
        .filter_map(|(_, function)| {
            matches!(function.kind, hir::FunctionKind::Intrinsic(_))
                .then_some(function.materialization)
        })
        .collect::<HashSet<_>>();
    for (_, reference) in source.callable_references.iter() {
        let intrinsic = match reference.target {
            hir::CallableReferenceTarget::BoundIntrinsic { .. } => true,
            _ => reference.target.callee().is_some_and(|target| {
                matches!(target,
                    hir::CallableTarget::Local(hir::Callable::Function(id))
                    if matches!(source.functions[id].kind, hir::FunctionKind::Intrinsic(_)))
            }),
        };
        if intrinsic {
            identities.insert(*reference.identity.materialization());
        }
    }
    let mut required = vec![false; module.functions.len()];
    for entry in module.meta.source_callable_materializations.iter() {
        required[index(entry.function())] = identities.contains(&entry.materialization());
    }
    for environment in &module.meta.closure_environments {
        if identities.contains(&environment.identity().callable()) {
            let class = &module.closure_classes[environment.class()];
            required[index(module.closure_invoke_functions[class.invoke].function)] = true;
            for bridge in &class.bridges {
                required[index(bridge.function)] = true;
            }
        }
    }
    for adjust in &module.meta.interface_adjusts {
        if match adjust.target() {
            mir::InterfaceAdjustTarget::Local(target) => required[index(target)],
            mir::InterfaceAdjustTarget::External(target) => external.contains(&target),
        } {
            required[index(adjust.function())] = true;
        }
    }
    Entries {
        local: required,
        external,
    }
}

pub(super) fn normalize(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    required: &Entries,
    dispatch: &crate::imported_bodies::IntrinsicDispatch,
    expansion: &mut inline::Session,
    registry: &mut LocalValueRegistry,
) -> Vec<bool> {
    let mut active = vec![true; module.functions.len()];
    let mut changed = vec![false; module.functions.len()];
    loop {
        let dispatched = super::devirtualize(module, selected, &active, Some(required), dispatch);
        let expanded = expansion.run_required(module, &active, registry);
        for (i, active) in active.iter_mut().enumerate() {
            *active = dispatched[i] || expanded[i];
            changed[i] |= *active;
        }
        if !active.iter().any(|active| *active) {
            return changed;
        }
    }
}
