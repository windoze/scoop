use std::collections::HashSet;

use crate::{Callee, Module, StatementKind};

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
            if module.meta.imported_dependency_callables.is_empty() =>
        {
            return Ok(Vec::new());
        }
        StrongImportedDependencyInput::Unused => {
            return Err(SingleConeStrongMirInputError::MissingImportedDependencyAuthority);
        }
        StrongImportedDependencyInput::Selected(_)
            if module.cone == scoop_identity::ConeIdentity::CORE =>
        {
            return Err(SingleConeStrongMirInputError::CoreCannotImportDependency);
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
    if module.meta.imported_dependency_callables.len() != selected.len() {
        return Err(
            SingleConeStrongMirInputError::ImportedDependencyCallableCountMismatch {
                module: module.meta.imported_dependency_callables.len(),
                selected: selected.len(),
            },
        );
    }

    let mut declarations = HashSet::with_capacity(selected.len());
    let mut roots = Vec::with_capacity(selected.len());
    for (callable, imported) in module.meta.imported_dependency_callables.iter() {
        let selected = selected.resolve_callable(imported.reference()).ok_or(
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
            gc_effect: imported.gc_effect(),
        });
    }

    let mut referenced = HashSet::with_capacity(roots.len());
    for (_, function) in module.functions.iter() {
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                let StatementKind::Call(effect) = &statement.kind else {
                    continue;
                };
                let call = match effect {
                    crate::CallEffect::Unit(call) | crate::CallEffect::Value { call, .. } => call,
                };
                if let Callee::DependencyStrong(callable) = call.target.callee {
                    referenced.insert(callable);
                }
            }
        }
    }
    for root in &roots {
        if !referenced.contains(&root.callable()) {
            return Err(
                SingleConeStrongMirInputError::UnreferencedImportedDependencyCallable {
                    index: root.callable().into_raw().into_u32(),
                },
            );
        }
    }
    Ok(roots)
}
