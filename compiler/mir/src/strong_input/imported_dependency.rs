use std::collections::HashSet;

use crate::Module;

use super::{
    SingleConeStrongMirInputError, StrongImportedDependencyCallableRoot,
    StrongImportedDependencyInput,
};

pub(super) fn validate_imported_dependency_callables(
    module: &Module,
    input: StrongImportedDependencyInput<'_>,
) -> Result<Vec<StrongImportedDependencyCallableRoot>, SingleConeStrongMirInputError> {
    let selected = match input {
        StrongImportedDependencyInput::Unused
            if module.meta.dependency_callables().next().is_none() =>
        {
            return Ok(Vec::new());
        }
        StrongImportedDependencyInput::Unused => {
            return Err(SingleConeStrongMirInputError::MissingImportedDependencyAuthority);
        }
        StrongImportedDependencyInput::Selected(selected) => selected,
    };
    if selected.consumer() != module.cone {
        return Err(
            SingleConeStrongMirInputError::ForeignImportedDependencySelection {
                expected: module.cone,
                actual: selected.consumer(),
            },
        );
    }
    if module.meta.dependency_callables().count() != selected.len() {
        return Err(
            SingleConeStrongMirInputError::ImportedDependencyCallableCountMismatch {
                module: module.meta.dependency_callables().count(),
                selected: selected.len(),
            },
        );
    }

    let mut declarations = HashSet::with_capacity(selected.len());
    let mut roots = Vec::with_capacity(selected.len());
    for (callable, reference) in module.meta.dependency_callables() {
        let selected = selected.resolve_callable(reference).ok_or(
            SingleConeStrongMirInputError::ForeignImportedDependencyCallable {
                index: callable.into_raw().into_u32(),
            },
        )?;
        let key = (selected.provider(), selected.declaration());
        if !declarations.insert(key) {
            return Err(
                SingleConeStrongMirInputError::DuplicateImportedDependencyCallable {
                    index: callable.into_raw().into_u32(),
                },
            );
        }
        roots.push(StrongImportedDependencyCallableRoot {
            callable,
            provider: selected.provider(),
            declaration: selected.declaration(),
            implementation: selected.implementation(),
            signature: selected.signature().clone(),
            gc_effect: module.meta.external_callables[callable].gc_effect(),
        });
    }

    Ok(roots)
}
