//! Executed singleton reads select their actual object and ensure definitions.

use std::collections::BTreeSet;

use scoop_hir::concrete::{self as hir, ExprKind, TypeKind};
use scoop_mir::{ParamFreeMirObjectValueV1, SelectedExternalMirCallable};

use super::{CrossConeMirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_dependency_singletons(
        &self,
        module: &hir::Module,
        callables: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<Vec<ParamFreeMirObjectValueV1>, Error> {
        let mut seen = BTreeSet::new();
        let mut objects = Vec::new();
        module
            .visit_executable_expressions(|occurrence| {
                let ExprKind::ImportedSingletonValue(value) = occurrence.expression.kind else {
                    return Ok(());
                };
                if !seen.insert(value) {
                    return Ok(());
                }
                let TypeKind::Class(class) = module.types[occurrence.expression.ty].kind else {
                    return Err(Error::SingletonType(value));
                };
                let provider = module.classes[class]
                    .origin
                    .source()
                    .ok_or(Error::SingletonType(value))?
                    .declaration()
                    .origin();
                let dependency = self
                    .provider(provider)
                    .ok_or(Error::MissingProvider { provider })?;
                let exports = dependency.production().layout().mir_type_bridge().exports();
                let object = exports
                    .objects()
                    .get(value)
                    .ok_or(Error::MissingSingleton { provider, value })?;
                if object.read().object()
                    != module.exact_type_identities[occurrence.expression.ty].id()
                {
                    return Err(Error::SingletonType(value));
                }
                let target = object.ensure();
                if !callables.iter().any(|callable| {
                    callable.provider() == provider && callable.implementation() == target
                }) {
                    let definition = exports
                        .callables()
                        .get(target)
                        .ok_or(Error::MissingExport { provider, target })?;
                    callables.push(SelectedExternalMirCallable::from_lowered(
                        provider,
                        definition.clone(),
                    ));
                }
                objects.push(object.clone());
                Ok(())
            })
            .map_err(|error| match error {
                hir::ExecutableExpressionVisitError::Structure(error) => Error::Expressions(error),
                hir::ExecutableExpressionVisitError::Visitor(error) => error,
            })?;
        Ok(objects)
    }
}
