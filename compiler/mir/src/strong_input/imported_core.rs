use std::collections::HashSet;

use crate::{Module, StatementKind};

use super::{
    SingleConeStrongMirInputError, StrongImportedCoreCallableRoot, StrongImportedCoreInput,
};

pub(super) fn validate_imported_core_callables(
    module: &Module,
    input: StrongImportedCoreInput<'_>,
) -> Result<Vec<StrongImportedCoreCallableRoot>, SingleConeStrongMirInputError> {
    let selected = match input {
        StrongImportedCoreInput::Unused if module.meta.imported_core_callables.is_empty() => {
            return Ok(Vec::new());
        }
        StrongImportedCoreInput::Unused => {
            return Err(SingleConeStrongMirInputError::MissingImportedCoreAuthority);
        }
        StrongImportedCoreInput::Selected(_)
            if module.cone == scoop_identity::ConeIdentity::CORE =>
        {
            return Err(SingleConeStrongMirInputError::CoreCannotImportCore);
        }
        StrongImportedCoreInput::Selected(selected) => selected,
    };
    if module.meta.imported_core_callables.len() != selected.len() {
        return Err(
            SingleConeStrongMirInputError::ImportedCoreCallableCountMismatch {
                module: module.meta.imported_core_callables.len(),
                selected: selected.len(),
            },
        );
    }

    let mut kinds = HashSet::with_capacity(selected.len());
    let mut roots = Vec::with_capacity(selected.len());
    for (callable, imported) in module.meta.imported_core_callables.iter() {
        let selected = selected.resolve_callable(imported.reference()).ok_or(
            SingleConeStrongMirInputError::ForeignImportedCoreCallable {
                index: callable.into_raw().into_u32(),
            },
        )?;
        if !kinds.insert(selected.kind()) {
            return Err(
                SingleConeStrongMirInputError::DuplicateImportedCoreCallable {
                    index: callable.into_raw().into_u32(),
                },
            );
        }
        if selected.signature().effect() != scoop_identity::Effect::Ordinary
            || selected.signature().receiver().is_present()
        {
            return Err(
                SingleConeStrongMirInputError::UnsupportedImportedCoreCallableShape {
                    index: callable.into_raw().into_u32(),
                },
            );
        }
        roots.push(StrongImportedCoreCallableRoot {
            callable,
            kind: selected.kind(),
            implementation: selected.implementation(),
            signature: selected.signature().clone(),
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
                if let crate::Callee::CoreExternal(callable) = call.target.callee {
                    referenced.insert(callable);
                }
            }
        }
    }
    for root in &roots {
        if !referenced.contains(&root.callable()) {
            return Err(
                SingleConeStrongMirInputError::UnreferencedImportedCoreCallable {
                    index: root.callable().into_raw().into_u32(),
                },
            );
        }
    }
    Ok(roots)
}
