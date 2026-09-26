//! Every selected callable resolves through its actual provider and role.

use scoop_lir::{SelectedDependencyLirCallableV1, SelectedExternalLirSet};
use scoop_mir::{SelectedDependencyMirCallableV1, SelectedExternalMirSet};

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
            if let Some(record) = callable.direct_record() {
                projected.push((callable.role(), self.project_selected_callable(record)?));
            }
        }
        SelectedExternalLirSet::try_from_role_records(self.current, projected)
            .map_err(Error::Selection)
    }

    fn project_selected_callable(
        &self,
        callable: &SelectedDependencyMirCallableV1,
    ) -> Result<SelectedDependencyLirCallableV1, Error> {
        let provider = callable.provider();
        let declaration = callable.declaration();
        let artifact = self
            .provider(provider)
            .ok_or(Error::MissingProvider { provider })?;
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
        Ok(SelectedDependencyLirCallableV1::from_export(
            provider,
            export.clone(),
        ))
    }
}
