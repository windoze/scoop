//! Select the defining Cone's storage ABI for each executable native address.

use scoop_hir::{DependencyHirOutput, concrete};
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    GeneratedCallableKey, PersistentGeneratedCallableId, StrongCallableDefinitionOwner,
};
use scoop_mir::{MirCallableLoweringRoleV1, SelectedExternalMirCallable};

use super::{CrossConeMirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_callback_storage(
        &self,
        hir: &DependencyHirOutput,
        selected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), Error> {
        let module = hir.output().local.module();
        let mut sources = std::collections::HashSet::new();
        module
            .visit_executable_expressions(|occurrence| {
                if let concrete::ExprKind::FunctionAddress(concrete::CallableTarget::Imported(
                    source,
                )) = occurrence.expression.kind
                {
                    sources.insert(source);
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .map_err(|error| match error {
                concrete::ExecutableExpressionVisitError::Structure(error) => {
                    Error::Expressions(error)
                }
                concrete::ExecutableExpressionVisitError::Visitor(never) => match never {},
            })?;
        for source in sources {
            let reference = module.imported_dependency_callables[source].reference();
            let source = hir
                .imported_dependencies()
                .resolve_callable(reference)
                .expect("the complete HIR output retains every executable address source");
            let provider = source.provider();
            let target = source.capability().implementation();
            let StrongCallableDefinitionOwner::Function(function) = target else {
                return Err(Error::SignatureMismatch { provider, target });
            };
            let role = GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                source: CallableMaterialization::new(
                    CallableTemplateOwner::Function(function),
                    CallableMaterializationContext::NoSubstitution,
                ),
                signature: source.capability().signature().clone(),
            };
            let bridge = PersistentGeneratedCallableId::from_key(&role)
                .map_err(|_| Error::SignatureMismatch { provider, target })?;
            let target = StrongCallableDefinitionOwner::GeneratedCallable(bridge);
            let artifact = self
                .provider(provider)
                .ok_or(Error::MissingProvider { provider })?;
            let definition = artifact
                .production()
                .layout()
                .mir_type_bridge()
                .exports()
                .callables()
                .get(target)
                .ok_or(Error::MissingExport { provider, target })?;
            if definition.lowering_role() != &MirCallableLoweringRoleV1::StaticCallbackStorage
                || definition.semantic_signature().exact() != source.capability().signature()
            {
                return Err(Error::SignatureMismatch { provider, target });
            }
            if !selected.iter().any(|callable| {
                callable.provider() == provider && callable.implementation() == target
            }) {
                selected.push(
                    SelectedExternalMirCallable::from_lowered(provider, definition.clone())
                        .map_err(|_| Error::SignatureMismatch { provider, target })?,
                );
            }
        }
        Ok(())
    }
}
