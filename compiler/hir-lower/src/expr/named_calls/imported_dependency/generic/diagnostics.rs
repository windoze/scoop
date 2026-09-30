//! Render failures from the original candidate transaction.

use super::*;
use crate::call_resolution::constraints::{ConstraintOrigin, TypeTerm};

impl Lowerer {
    pub(super) fn imported_generic_inference_error(
        &mut self,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        signature: &LoadedCallableSignature,
        failure: &crate::call_resolution::constraints::ConstraintFailure,
    ) {
        let message = self.render_imported_constraint_failure(
            &signature.signature.owner_parameters,
            &signature.signature.callable_parameters,
            failure,
        );
        let span = match failure.origin {
            ConstraintOrigin::Argument(input) => call.arguments.span(input.index()),
            ConstraintOrigin::ExplicitTypeArgument(index) => call.type_args[index as usize].span(),
            _ => call.span,
        };
        self.error(
            span,
            format!("dependency function `{}`: {message}", name.text),
        );
    }

    pub(in crate::expr) fn render_imported_constraint_failure(
        &self,
        owner_parameters: &[hir::TypeParamDecl],
        callable_parameters: &[hir::TypeParamDecl],
        failure: &crate::call_resolution::constraints::ConstraintFailure,
    ) -> String {
        use crate::call_resolution::constraints::ConstraintFailureKind as Kind;
        let all_parameters = owner_parameters
            .iter()
            .chain(callable_parameters)
            .cloned()
            .collect::<Vec<_>>();
        let parameter = |variable: crate::call_resolution::constraints::InferenceVariableId| {
            let parameters = match variable {
                crate::call_resolution::constraints::InferenceVariableId::Owner(_) => {
                    owner_parameters
                }
                crate::call_resolution::constraints::InferenceVariableId::Callable(_) => {
                    callable_parameters
                }
            };
            &parameters[variable.group_index()].name
        };
        let type_term = |term: TypeTerm| match term {
            TypeTerm::Variable(variable) => parameter(variable).clone(),
            TypeTerm::Type(ty) | TypeTerm::Rigid(ty) => {
                self.type_name_with_params(ty, &all_parameters)
            }
        };
        match &failure.kind {
            Kind::Kind {
                variable,
                solution,
                required,
            } => format!(
                "type argument `{}` for `{}` must satisfy `{}`",
                self.type_name(*solution),
                parameter(*variable),
                match required {
                    hir::TypeParamKind::Any => "any",
                    hir::TypeParamKind::Value => "value",
                    hir::TypeParamKind::Ref => "ref",
                }
            ),
            Kind::ClassBound {
                variable,
                solution,
                required,
            }
            | Kind::InterfaceBound {
                variable,
                solution,
                required,
            } => format!(
                "type argument `{}` for `{}` must satisfy upper bound `{}`",
                self.type_name(*solution),
                parameter(*variable),
                self.type_name(*required)
            ),
            Kind::UnresolvedTerm(TypeTerm::Variable(variable))
            | Kind::NoUniqueSolution { variable, .. } => format!(
                "cannot infer a unique type argument for `{}`",
                parameter(*variable)
            ),
            Kind::ConflictingExactBounds {
                variable,
                first,
                second,
            } => format!(
                "conflicting types for `{}`: {} and {}",
                parameter(*variable),
                self.type_name(*first),
                self.type_name(*second)
            ),
            Kind::Relation {
                relation,
                left,
                right,
            } => format!(
                "{} {} {}",
                type_term(*left),
                match relation {
                    crate::call_resolution::constraints::RelationKind::Equal => "is not equal to",
                    crate::call_resolution::constraints::RelationKind::Subtype =>
                        "is not a subtype of",
                },
                type_term(*right),
            ),
            Kind::CallableShape(mismatch) => match mismatch {
                crate::call_resolution::constraints::CallableShapeMismatch::ExpectedCallable => {
                    "expected a callable type".into()
                }
                crate::call_resolution::constraints::CallableShapeMismatch::Suspend => {
                    "ordinary and suspend callable shapes differ".into()
                }
                crate::call_resolution::constraints::CallableShapeMismatch::Arity {
                    expected,
                    actual,
                } => format!(
                    "callable shape expects {expected} parameter(s), but the actual type has {actual}"
                ),
            },
            Kind::ForeignVariable(_) => {
                "candidate references an inference variable from another session".into()
            }
            Kind::ForeignTypeParameter(_) => {
                "candidate references a type parameter outside its declaration".into()
            }
            Kind::UnresolvedTerm(term) => format!("cannot resolve type term {}", type_term(*term)),
            Kind::NonConcreteApplication(_) => {
                "candidate result is not a complete concrete type application".into()
            }
        }
    }
}
