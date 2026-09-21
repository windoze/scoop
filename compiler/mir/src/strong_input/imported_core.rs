use std::collections::HashSet;

use crate::Module;

use super::{
    SingleConeStrongMirInputError, StrongImportedCoreCallableRoot, StrongImportedCoreInput,
};

pub(super) fn validate_imported_core_callables(
    module: &Module,
    input: StrongImportedCoreInput<'_>,
) -> Result<Vec<StrongImportedCoreCallableRoot>, SingleConeStrongMirInputError> {
    let selected = match input {
        StrongImportedCoreInput::Unused
            if module.meta.initialization_callables().next().is_none() =>
        {
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
    if module.meta.initialization_callables().count() != selected.len() {
        return Err(
            SingleConeStrongMirInputError::ImportedCoreCallableCountMismatch {
                module: module.meta.initialization_callables().count(),
                selected: selected.len(),
            },
        );
    }

    let mut kinds = HashSet::with_capacity(selected.len());
    let mut roots = Vec::with_capacity(selected.len());
    for (callable, reference) in module.meta.initialization_callables() {
        let selected = selected.resolve_callable(reference).ok_or(
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

    Ok(roots)
}
