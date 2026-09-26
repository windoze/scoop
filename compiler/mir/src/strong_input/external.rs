use std::collections::HashSet;

use super::{
    SingleConeStrongMirInputError as Error, StrongExternalCallableInput, StrongExternalCallableRoot,
};
use crate::{CallableRole, GcEffect, Module};

/// Resolves every MIR use through one complete selection before projecting the
/// existing LIR metadata roles. Both output sealing paths use this validator.
pub(crate) fn validate_external_callables(
    module: &Module,
    input: StrongExternalCallableInput<'_>,
) -> Result<Vec<StrongExternalCallableRoot>, Error> {
    let selected = match input {
        StrongExternalCallableInput::Unused if module.meta.external_callables.is_empty() => {
            return Ok(Vec::new());
        }
        StrongExternalCallableInput::Unused => return Err(Error::MissingExternalCallableSelection),
        StrongExternalCallableInput::Selected(selected) => selected,
    };
    if selected.consumer() != module.cone {
        return Err(Error::ForeignExternalCallableSelection {
            expected: module.cone,
            actual: selected.consumer(),
        });
    }
    if module.meta.external_callables.len() != selected.len() {
        return Err(Error::ExternalCallableCountMismatch {
            module: module.meta.external_callables.len(),
            selected: selected.len(),
        });
    }
    let referenced = crate::external_callable::referenced_external_callables(module);
    let mut implementations = HashSet::new();
    let mut roots = Vec::with_capacity(selected.len());
    for (callable, value) in module.meta.external_callables.iter() {
        let index = callable.into_raw().into_u32();
        let selected = selected
            .resolve_callable(value.reference())
            .ok_or(Error::ForeignExternalCallable { index })?;
        if !implementations.insert(selected.implementation()) {
            return Err(Error::DuplicateExternalImplementation {
                implementation: selected.implementation(),
            });
        }
        if !referenced.contains(&callable) {
            return Err(Error::UnreferencedExternalCallable { index });
        }
        if selected.role() == CallableRole::InitializationCycle
            && value.gc_effect() != GcEffect::Managed
        {
            return Err(Error::InitializationCycleGcEffect { index });
        }
        roots.push(StrongExternalCallableRoot {
            callable,
            role: selected.role(),
            provider: selected.provider(),
            implementation: selected.implementation(),
            signature: selected.signature().clone(),
            gc_effect: value.gc_effect(),
        });
    }
    Ok(roots)
}
