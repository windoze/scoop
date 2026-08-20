use super::*;

pub(super) fn exclusive_calls(
    module: &hir::Module,
) -> Result<
    std::collections::HashSet<hir::ImportedDependencyCallableUseId>,
    CurrentConeMirLoweringError,
> {
    let mut release = std::collections::HashSet::new();
    if module.release_hooks.is_empty() {
        return Ok(release);
    }
    let mut ordinary = std::collections::HashSet::new();
    module
        .visit_executable_expressions(|occurrence| {
            let callable = if let Some(hir::LiteralPatternEquality::Ordinary {
                equals: hir::CallableTarget::Imported(callable),
            }) = occurrence.literal_equality
            {
                Some(callable)
            } else {
                match &occurrence.expression.kind {
                    hir::ExprKind::Call {
                        callee: hir::CallableTarget::Imported(callable),
                        ..
                    }
                    | hir::ExprKind::FunctionAddress(hir::CallableTarget::Imported(callable))
                    | hir::ExprKind::ClassInitializerCall {
                        initializer: hir::ClassInitializerTarget::Imported(callable),
                        ..
                    } => Some(*callable),
                    hir::ExprKind::CallableReference(id) => {
                        match module.callable_references[*id].target.callee() {
                            Some(hir::CallableTarget::Imported(callable)) => Some(callable),
                            _ => None,
                        }
                    }
                    _ => None,
                }
            };
            if let Some(callable) = callable {
                if matches!(
                    occurrence.position.root.template(),
                    hir::CallableTemplateOwner::ReleaseHook(_)
                ) {
                    release.insert(callable);
                } else {
                    ordinary.insert(callable);
                }
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .map_err(|error| {
            CurrentConeMirLoweringError::Occurrences(match error {
                hir::ExecutableExpressionVisitError::Structure(error) => {
                    scoop_hir::DependencyCallOccurrenceError::Structure(error)
                }
                hir::ExecutableExpressionVisitError::Visitor(error) => match error {},
            })
        })?;
    release.retain(|callable| !ordinary.contains(callable));
    Ok(release)
}
