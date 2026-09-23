//! Stage-specific projection of committed dependency selections.

use scoop_hir::SelectedImportedDependencySet;
use scoop_mir::{SelectedDependencyMirCallableV1, SelectedExternalMirSet};

use super::{ValidatedCrossConeSemanticClosure, world::provider_certificate};

mod descriptor;
mod errors;
mod lir;
mod protocols;
pub use descriptor::CrossConeTypeDescriptorProjectionError;
pub use errors::*;
pub use protocols::{CrossConeInitializationSelectionError, CrossConeProtocolImportError};

impl ValidatedCrossConeSemanticClosure<'_> {
    /// Projects the exact HIR winners into the matching provider MIR exports.
    ///
    /// Constants have already been inlined into HIR and therefore do not
    /// produce MIR roots. Every callable is checked against both its retained
    /// artifact certificate and the terminal provider's canonical export.
    pub fn project_dependency_callables_to_mir(
        &self,
        selected: &SelectedImportedDependencySet,
    ) -> Result<SelectedExternalMirSet, CrossConeMirSelectionProjectionError> {
        if selected.consumer() != self.current {
            return Err(CrossConeMirSelectionProjectionError::ConsumerMismatch {
                closure: self.current,
                selected: selected.consumer(),
            });
        }

        let mut projected = Vec::with_capacity(selected.callable_count());
        for callable in selected.callables() {
            let provider = callable.provider();
            let artifact = self
                .provider(provider)
                .ok_or(CrossConeMirSelectionProjectionError::MissingProvider { provider })?;
            if callable.certificate() != &provider_certificate(artifact) {
                return Err(
                    CrossConeMirSelectionProjectionError::ProviderCertificateMismatch { provider },
                );
            }

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
