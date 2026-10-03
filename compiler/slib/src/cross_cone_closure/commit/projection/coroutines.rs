//! Select only startup helpers actually invoked by executable HIR.

use scoop_hir::{DependencyHirOutput, concrete};
use scoop_identity::{
    Effect, ExactCallableSignature, ExactTypeKey, GeneratedCallableKey,
    PersistentGeneratedCallableId, StrongCallableDefinitionOwner,
};
use scoop_mir::{MirCallableLoweringRoleV1, SelectedExternalMirCallable};

use super::{CrossConeMirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_coroutine_starts(
        &self,
        hir: &DependencyHirOutput,
        selected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), Error> {
        let module = hir.output().local.module();
        let mut starts = std::collections::BTreeSet::new();
        module.visit_executable_expressions(|occurrence| {
            if let concrete::ExprKind::Call {
                callee: concrete::CallableTarget::Local(callable), ..
            } = occurrence.expression.kind {
                let function = module.callable_function(callable);
                if matches!(module.functions[function].kind, concrete::FunctionKind::Intrinsic(intrinsic)
                    if intrinsic.kind == concrete::IntrinsicFunctionKind::CoroutineStart)
                {
                    starts.insert(function);
                }
            }
            Ok::<_, std::convert::Infallible>(())
        }).map_err(|error| match error {
            concrete::ExecutableExpressionVisitError::Structure(error) => Error::Expressions(error),
            concrete::ExecutableExpressionVisitError::Visitor(never) => match never {},
        })?;
        for start in starts {
            let protocol = module
                .coroutine_protocol_for_function(start)
                .expect("a startup intrinsic retains its complete concrete protocol");
            let result = &module.exact_type_identities[protocol.result_type];
            let ExactTypeKey::Nominal(source) = result.key() else {
                continue;
            };
            let provider = self.identity_inputs().find_map(|(_, identities)| {
                identities
                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(*source)
                    .ok()
                    .map(|declaration| declaration.origin())
            });
            let Some(provider) = provider.filter(|provider| *provider != module.cone) else {
                continue;
            };
            let callable =
                PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::CoroutineStart {
                    result: result.id(),
                })
                .expect("the exact result defines one canonical startup helper");
            let target = StrongCallableDefinitionOwner::GeneratedCallable(callable);
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
            let function = &module.functions[start];
            let signature = ExactCallableSignature::new(
                Effect::Ordinary,
                None,
                function
                    .params
                    .iter()
                    .map(|parameter| module.exact_type_identities[parameter.ty].id())
                    .collect(),
                module.exact_type_identities[function.return_ty].id(),
            );
            if definition.lowering_role() != &MirCallableLoweringRoleV1::CoroutineStart
                || definition.semantic_signature().exact() != &signature
            {
                return Err(Error::SignatureMismatch { provider, target });
            }
            selected.push(
                SelectedExternalMirCallable::from_lowered(provider, definition.clone())
                    .map_err(|_| Error::SignatureMismatch { provider, target })?,
            );
        }
        Ok(())
    }
}
