//! Implicit runtime failures select ordinary dependency constructors.

use scoop_hir::{ImportedCoreProtocolCallableDefinition, concrete};
use scoop_identity::StrongCallableDefinitionOwner;
use scoop_mir::{MirCallableLoweringRoleV1, SelectedExternalMirCallable};

use super::{CrossConeMirSelectionProjectionError as Error, ValidatedCrossConeSemanticClosure};

impl ValidatedCrossConeSemanticClosure {
    pub(super) fn project_runtime_constructors(
        &self,
        module: &concrete::Module,
        selected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), Error> {
        let concrete::ConcreteCoreProtocols::Imported(protocols) = &module.core_protocols else {
            return Ok(());
        };
        let mut cast_required = false;
        let mut arithmetic_required = false;
        let mut unwrap_required = false;
        let mut bounds_required = false;
        let mut coroutine_required = false;
        module
            .visit_executable_expressions(|occurrence| {
                coroutine_required |= requires_coroutine_state(module, occurrence.expression);
                unwrap_required |= matches!(
                    occurrence.expression.kind,
                    concrete::ExprKind::Unwrap {
                        trap_on_none: true,
                        ..
                    }
                );
                cast_required |= matches!(
                    occurrence.expression.kind,
                    concrete::ExprKind::Cast {
                        optional: false,
                        ..
                    }
                );
                arithmetic_required |= matches!(
                    occurrence.expression.kind,
                    concrete::ExprKind::IntegerOperation {
                        operation: concrete::IntegerOperation::Managed { .. },
                        ..
                    }
                );
                bounds_required |= matches!(
                    occurrence.expression.kind,
                    concrete::ExprKind::Index { .. } | concrete::ExprKind::ArraySet { .. }
                );
                if let concrete::ExprKind::CallableReference(id) = occurrence.expression.kind {
                    arithmetic_required |= matches!(
                        module.callable_references[id].target,
                        concrete::CallableReferenceTarget::BoundIntrinsic { intrinsic, .. }
                            if intrinsic.requires_arithmetic_exception()
                    );
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .map_err(|error| match error {
                concrete::ExecutableExpressionVisitError::Structure(error) => {
                    Error::Expressions(error)
                }
                concrete::ExecutableExpressionVisitError::Visitor(never) => match never {},
            })?;
        for (required, constructor) in [
            (
                coroutine_required,
                protocols.exceptions().illegal_state_exception_constructor(),
            ),
            (
                unwrap_required,
                protocols.exceptions().unwrap_exception_constructor(),
            ),
            (
                cast_required,
                protocols.exceptions().class_cast_exception_constructor(),
            ),
            (
                arithmetic_required,
                protocols.exceptions().arithmetic_exception_constructor(),
            ),
            (
                bounds_required,
                protocols
                    .exceptions()
                    .index_out_of_bounds_exception_constructor(),
            ),
        ] {
            if required {
                self.project_runtime_constructor(constructor, selected)?;
            }
        }
        Ok(())
    }

    fn project_runtime_constructor(
        &self,
        constructor: &scoop_hir::ImportedCoreProtocolCallable,
        selected: &mut Vec<SelectedExternalMirCallable>,
    ) -> Result<(), Error> {
        let provider = constructor.provider();
        let target = match constructor.definition() {
            ImportedCoreProtocolCallableDefinition::Constructor(id) => {
                StrongCallableDefinitionOwner::Constructor(id.persistent())
            }
            ImportedCoreProtocolCallableDefinition::GeneratedCallable(id) => {
                StrongCallableDefinitionOwner::GeneratedCallable(id.persistent())
            }
            definition => return Err(Error::RuntimeConstructorKind(definition)),
        };
        if selected
            .iter()
            .any(|callable| callable.provider() == provider && callable.implementation() == target)
        {
            return Ok(());
        }
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
        if !matches!(
            definition.lowering_role(),
            MirCallableLoweringRoleV1::ClassInitializer { .. }
        ) || !definition
            .semantic_signature()
            .exact()
            .parameters()
            .is_empty()
        {
            return Err(Error::SignatureMismatch { provider, target });
        }
        selected.push(
            SelectedExternalMirCallable::from_lowered(provider, definition.clone())
                .map_err(|_| Error::SignatureMismatch { provider, target })?,
        );
        Ok(())
    }
}

fn requires_coroutine_state(module: &concrete::Module, expression: &concrete::Expr) -> bool {
    let function_type = match expression.kind {
        concrete::ExprKind::Call { callee, .. } => {
            return match callee {
                concrete::CallableTarget::DerivedEquality(_) => false,
                concrete::CallableTarget::Local(callable) => {
                    module.functions[module.callable_function(callable)].is_suspend
                }
                concrete::CallableTarget::Imported(callable) => {
                    module.imported_dependency_callables[callable].effect()
                        == scoop_identity::Effect::Suspend
                }
            };
        }
        concrete::ExprKind::CallableCall { function_type, .. } => {
            return module.function_types[function_type].is_suspend;
        }
        concrete::ExprKind::MethodCall { callee, .. }
        | concrete::ExprKind::DirectSuperMethodCall { callee, .. } => {
            return module.functions[module.callable_function(callee)].is_suspend;
        }
        concrete::ExprKind::Lambda(_)
        | concrete::ExprKind::AnonymousFunction(_)
        | concrete::ExprKind::CallableReference(_)
        | concrete::ExprKind::FunctionCoercion { .. } => expression.ty,
        concrete::ExprKind::Cast { check_ty, .. } => check_ty,
        _ => return false,
    };
    match module.types[function_type].kind {
        concrete::TypeKind::Function(function) => module.function_types[function].is_suspend,
        _ => false,
    }
}
