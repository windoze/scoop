//! Stage-specific projection of committed dependency selections.

use scoop_hir::DependencyHirOutput;
use scoop_mir::{
    SelectedDependencyMirCallableV1, SelectedExternalMirCallable, SelectedExternalMirSet,
};
use scoop_wire::WirePath;

use super::ValidatedCrossConeSemanticClosure;

mod errors;
mod initialization;
mod lir;
mod protocols;
mod runtime_constructors;
pub use errors::*;
pub use protocols::CrossConeProtocolImportError;

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
            let target = capability.implementation();
            let expected_gc = match capability.gc_effect() {
                scoop_identity::GcEffect::Managed => scoop_mir::GcEffect::Managed,
                scoop_identity::GcEffect::NoGc => scoop_mir::GcEffect::NoGc,
            };
            if let Some(export) = capability
                .direct_declaration()
                .and_then(|declaration| artifact.production().mir_cross_cone().export(declaration))
            {
                if export.implementation() != target {
                    return Err(
                        CrossConeMirSelectionProjectionError::ImplementationMismatch {
                            provider,
                            target,
                        },
                    );
                }
                if export.signature() != capability.signature() || export.gc_effect() != expected_gc
                {
                    return Err(CrossConeMirSelectionProjectionError::SignatureMismatch {
                        provider,
                        target,
                    });
                }
                let record = SelectedDependencyMirCallableV1::try_new(
                    provider,
                    export.declaration(),
                    target,
                    export.signature().clone(),
                )
                .map_err(CrossConeMirSelectionProjectionError::Record)?;
                projected.push(SelectedExternalMirCallable::dependency(record));
            } else {
                let definition = artifact
                    .production()
                    .layout()
                    .mir_type_bridge()
                    .exports()
                    .callables()
                    .get(target)
                    .ok_or(CrossConeMirSelectionProjectionError::MissingExport {
                        provider,
                        target,
                    })?;
                if definition.semantic_signature().exact() != capability.signature()
                    || definition.semantic_signature().gc_effect() != expected_gc
                {
                    return Err(CrossConeMirSelectionProjectionError::SignatureMismatch {
                        provider,
                        target,
                    });
                }
                projected.push(SelectedExternalMirCallable::from_lowered(
                    provider,
                    definition.clone(),
                ));
            }
        }

        self.project_initialization_callables(hir.output().local.module(), &mut projected)?;
        self.project_runtime_constructors(hir.output().local.module(), &mut projected)?;
        SelectedExternalMirSet::try_from_selections(self.current, projected)
            .map_err(CrossConeMirSelectionProjectionError::Selection)
    }
}
