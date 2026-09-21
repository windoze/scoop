use std::collections::HashSet;

use super::{
    SingleConeStrongMirInputError as Error, StrongExternalCallableInput,
    StrongImportedCoreCallableRoot, StrongImportedDependencyCallableRoot,
};
use crate::{ExternalCallableRole, GcEffect, Module};

/// Resolves every MIR use through one complete selection before projecting the
/// existing LIR metadata roles. Both output sealing paths use this validator.
pub(crate) fn validate_external_callables(
    module: &Module,
    input: StrongExternalCallableInput<'_>,
) -> Result<
    (
        Vec<StrongImportedCoreCallableRoot>,
        Vec<StrongImportedDependencyCallableRoot>,
    ),
    Error,
> {
    let selected = match input {
        StrongExternalCallableInput::Unused if module.meta.external_callables.is_empty() => {
            return Ok((Vec::new(), Vec::new()));
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
    let mut declarations = HashSet::new();
    let mut protocols = Vec::new();
    let mut dependencies = Vec::new();
    for (callable, value) in module.meta.external_callables.iter() {
        let index = callable.into_raw().into_u32();
        let selected = selected
            .resolve_callable(value.reference())
            .ok_or(Error::ForeignExternalCallable { index })?;
        if !declarations.insert((selected.provider(), selected.declaration())) {
            return Err(Error::DuplicateExternalCallable { index });
        }
        if !implementations.insert(selected.implementation()) {
            return Err(Error::DuplicateExternalImplementation {
                implementation: selected.implementation(),
            });
        }
        if !referenced.contains(&callable) {
            return Err(Error::UnreferencedExternalCallable { index });
        }
        match selected.role() {
            ExternalCallableRole::InitializationCycle => {
                if value.gc_effect() != GcEffect::Managed {
                    return Err(Error::InitializationCycleGcEffect { index });
                }
                protocols.push(StrongImportedCoreCallableRoot {
                    callable,
                    kind: scoop_identity::CoreImportedCallableKind::InitializationCycleThrower,
                    implementation: selected.implementation(),
                    signature: selected.signature().clone(),
                });
            }
            ExternalCallableRole::Dependency => {
                dependencies.push(StrongImportedDependencyCallableRoot {
                    callable,
                    provider: selected.provider(),
                    declaration: selected.declaration(),
                    implementation: selected.implementation(),
                    signature: selected.signature().clone(),
                    gc_effect: value.gc_effect(),
                })
            }
        }
    }
    Ok((protocols, dependencies))
}
