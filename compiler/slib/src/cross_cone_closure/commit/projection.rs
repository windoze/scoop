//! Stage-specific projection of committed dependency selections.

use scoop_hir::DependencyHirOutput;
use scoop_mir::{SelectedDependencyMirCallableV1, SelectedExternalMirSet};
use scoop_wire::WirePath;

use super::ValidatedCrossConeSemanticClosure;

mod errors;
mod lir;
mod protocols;
pub use errors::*;
pub use protocols::{CrossConeInitializationSelectionError, CrossConeProtocolImportError};

impl ValidatedCrossConeSemanticClosure {
    /// Projects actual executable HIR calls into matching provider MIR exports.
    ///
    /// Constants have already been inlined into HIR and therefore do not
    /// produce MIR roots. Callables use the actual provider's complete export
    /// and retain the checked implementation and signature.
    pub fn project_dependency_callables_to_mir(
        &self,
        hir: &DependencyHirOutput,
    ) -> Result<SelectedExternalMirSet, CrossConeMirSelectionProjectionError> {
        let consumer = hir.output().local.module().cone;
        if consumer != self.current {
            return Err(CrossConeMirSelectionProjectionError::ConsumerMismatch {
                closure: self.current,
                selected: consumer,
            });
        }

        let selected = hir
            .executable_dependency_callables()
            .map_err(CrossConeMirSelectionProjectionError::Occurrences)?;
        let mut projected = Vec::new();

        scoop_wire::allocation::try_reserve(&mut projected, selected.len(), &WirePath::root())
            .map_err(CrossConeMirSelectionProjectionError::Resource)?;
        for use_ in selected {
            let callable = use_.callable();
            let provider = callable.provider();
            let artifact = self
                .provider(provider)
                .ok_or(CrossConeMirSelectionProjectionError::MissingProvider { provider })?;
            let capability = callable.capability();
            let declaration = capability.declaration();
            let export = artifact
                .production()
                .mir_cross_cone()
                .export(declaration)
                .ok_or(CrossConeMirSelectionProjectionError::MissingExport {
                    provider,
                    declaration,
                })?;
            if export.implementation() != capability.implementation() {
                return Err(
                    CrossConeMirSelectionProjectionError::ImplementationMismatch {
                        provider,
                        declaration,
                    },
                );
            }
            if export.signature() != capability.signature() {
                return Err(CrossConeMirSelectionProjectionError::SignatureMismatch {
                    provider,
                    declaration,
                });
            }
            projected.push(
                SelectedDependencyMirCallableV1::try_new(
                    provider,
                    declaration,
                    export.implementation(),
                    export.signature().clone(),
                )
                .map_err(CrossConeMirSelectionProjectionError::Record)?,
            );
        }

        SelectedExternalMirSet::try_from_callables(self.current, projected)
            .map_err(CrossConeMirSelectionProjectionError::Selection)
    }
}
