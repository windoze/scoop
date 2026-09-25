//! Every selected callable resolves through its actual provider and role.

use scoop_identity::{CallableRole, DependencyCallableDeclarationId};
use scoop_lir::{SelectedDependencyLirCallableV1, SelectedExternalLirSet};
use scoop_mir::{SelectedExternalMirCallable, SelectedExternalMirSet};

use super::{CrossConeLirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    /// Projects the complete MIR selection through the same committed closure.
    pub fn project_dependency_callables_to_lir(
        &self,
        selected: &SelectedExternalMirSet,
    ) -> Result<SelectedExternalLirSet, Error> {
        if selected.consumer() != self.current {
            return Err(Error::ConsumerMismatch {
                closure: self.current,
                selected: selected.consumer(),
            });
        }
        let mut projected = Vec::with_capacity(selected.len());
        for callable in selected.callables() {
            projected.push((callable.role(), self.project_selected_callable(callable)?));
        }
        SelectedExternalLirSet::try_from_role_records(self.current, projected)
            .map_err(Error::Selection)
    }

    fn project_selected_callable(
        &self,
        callable: &SelectedExternalMirCallable,
    ) -> Result<SelectedDependencyLirCallableV1, Error> {
        let provider = callable.provider();
        let declaration = callable.declaration();
        let artifact = self
            .provider(provider)
            .ok_or(Error::MissingProvider { provider })?;
        match callable.role() {
            CallableRole::Ordinary => {
                let export = artifact
                    .production()
                    .lir_cross_cone()
                    .export(declaration)
                    .ok_or(Error::MissingExport {
                        provider,
                        declaration,
                    })?;
                if export.target() != callable.implementation()
                    || export.abi_signature().signature() != callable.signature()
                {
                    return Err(Error::BridgeMismatch {
                        provider,
                        declaration,
                    });
                }
                let selected = SelectedDependencyLirCallableV1::new(
                    provider,
                    declaration,
                    export.target(),
                    export.abi_signature().clone(),
                    export.calling_convention(),
                    export.root_plan(),
                )
                .map_err(Error::Record)?;
                if selected.bridge() != export {
                    return Err(Error::BridgeMismatch {
                        provider,
                        declaration,
                    });
                }
                Ok(selected)
            }
            CallableRole::InitializationCycle => {
                let source = artifact
                    .production()
                    .mir_core()
                    .strong_callable_bridges()
                    .get(callable.implementation().callable_owner())
                    .ok_or(Error::MissingExport {
                        provider,
                        declaration,
                    })?;
                if source.role() != callable.role() {
                    return Err(Error::RoleMismatch {
                        provider,
                        declaration,
                        selected: callable.role(),
                        actual: source.role(),
                    });
                }
                if source.signature() != callable.signature()
                    || !matches!(declaration, DependencyCallableDeclarationId::Function(_))
                {
                    return Err(Error::BridgeMismatch {
                        provider,
                        declaration,
                    });
                }
                let production = artifact.production().lir_strong();
                let abi = production
                    .initialization_cycle_abi()
                    .ok_or(Error::MissingInitializationAbi { provider })?;
                artifact
                    .lir()
                    .project_initialization_cycle_thrower(
                        abi,
                        production.canonical_definitions(),
                        callable.implementation(),
                        callable.signature().clone(),
                    )
                    .map_err(Error::InitializationService)
            }
        }
    }
}
