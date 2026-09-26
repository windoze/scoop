//! Initialization units select their actual dependency functions.

use scoop_hir::{ImportedCoreProtocolCallableDefinition, concrete};
use scoop_identity::{DependencyCallableDeclarationId, StrongCallableDefinitionOwner};
use scoop_mir::{SelectedDependencyMirCallableV1, SelectedExternalMirCallable};

use super::{CrossConeMirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_initialization_callables(
        &self,
        module: &concrete::Module,
        selected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), Error> {
        for (_, unit) in module.initialization_units.iter() {
            let concrete::InitializationCycleThrower::Imported(callable) = &unit.cycle_thrower
            else {
                continue;
            };
            let ImportedCoreProtocolCallableDefinition::Function(definition) =
                callable.definition()
            else {
                return Err(Error::InitializationFunctionKind(callable.definition()));
            };
            let provider = callable.provider();
            let target = StrongCallableDefinitionOwner::Function(definition.persistent());
            if selected.iter().any(|callable| {
                callable.provider() == provider && callable.implementation() == target
            }) {
                continue;
            }
            let artifact = self
                .provider(provider)
                .ok_or(Error::MissingProvider { provider })?;
            let export = artifact
                .production()
                .mir_cross_cone()
                .export(DependencyCallableDeclarationId::Function(
                    definition.persistent(),
                ))
                .ok_or(Error::MissingExport { provider, target })?;
            if export.implementation() != target {
                return Err(Error::ImplementationMismatch { provider, target });
            }
            let record = SelectedDependencyMirCallableV1::try_new(
                provider,
                export.declaration(),
                target,
                export.signature().clone(),
            )
            .map_err(Error::Record)?;
            selected.push(SelectedExternalMirCallable::dependency(record));
        }
        Ok(())
    }
}
