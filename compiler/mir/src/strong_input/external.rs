use std::collections::HashSet;

use super::{
    SingleConeStrongMirInputError, StrongImportedCoreCallableRoot, StrongImportedCoreInput,
    StrongImportedDependencyCallableRoot, StrongImportedDependencyInput,
    imported_core::validate_imported_core_callables,
    imported_dependency::validate_imported_dependency_callables,
};
use crate::Module;

pub(crate) fn validate_external_callables(
    module: &Module,
    protocols: StrongImportedCoreInput<'_>,
    dependencies: StrongImportedDependencyInput<'_>,
) -> Result<
    (
        Vec<StrongImportedCoreCallableRoot>,
        Vec<StrongImportedDependencyCallableRoot>,
    ),
    SingleConeStrongMirInputError,
> {
    let core = validate_imported_core_callables(module, protocols)?;
    let dependencies = validate_imported_dependency_callables(module, dependencies)?;
    let mut implementations = HashSet::with_capacity(module.meta.external_callables.len());
    for implementation in core
        .iter()
        .map(|root| root.implementation())
        .chain(dependencies.iter().map(|root| root.implementation()))
    {
        if !implementations.insert(implementation) {
            return Err(
                SingleConeStrongMirInputError::DuplicateExternalImplementation { implementation },
            );
        }
    }
    let referenced = crate::external_callable::referenced_external_callables(module);
    for (id, _) in module.meta.external_callables.iter() {
        if !referenced.contains(&id) {
            return Err(
                SingleConeStrongMirInputError::UnreferencedExternalCallable {
                    index: id.into_raw().into_u32(),
                },
            );
        }
    }
    Ok((core, dependencies))
}
