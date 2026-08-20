//! Initialization units select their actual dependency functions.

use scoop_hir::{ImportedCoreProtocolCallableDefinition, concrete};
use scoop_identity::{DependencyCallableDeclarationId, StrongCallableDefinitionOwner};
use scoop_mir::{SelectedDependencyMirCallableV1, SelectedExternalMirCallable};

use super::{CrossConeMirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_core_functions(
        &self,
        output: &scoop_hir::LocalConcreteHirOutput,
        selected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), Error> {
        let module = output.module();
        let arguments = match output.output_kind() {
            scoop_hir::LocalConeOutputKind::Executable { local_entry }
                if !module.functions[local_entry.local_function().function()]
                    .params
                    .is_empty() =>
            {
                match &module.core_protocols {
                    concrete::ConcreteCoreProtocols::Imported(protocols) => {
                        Some(protocols.program_arguments())
                    }
                    concrete::ConcreteCoreProtocols::Defined(_) => None,
                }
            }
            _ => None,
        };
        let callables = module
            .initialization_units
            .iter()
            .filter_map(|(_, unit)| match &unit.cycle_thrower {
                concrete::InitializationCycleThrower::Imported(callable) => Some(callable),
                concrete::InitializationCycleThrower::Local(_) => None,
            })
            .chain(arguments);
        for callable in callables {
            let ImportedCoreProtocolCallableDefinition::Function(definition) =
                callable.definition()
            else {
                return Err(Error::CoreFunctionKind(callable.definition()));
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
