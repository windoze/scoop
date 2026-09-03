use super::*;

use crate::call_resolution::constraints::{
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin, InferenceVariableId, TypeTerm,
};
use crate::overload::probe::{CandidateProbeFailure, CandidateProbeFailureKind};

impl Lowerer {
    pub(super) fn candidate_failures_diagnostic(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        failures: &mut [CandidateProbeFailure],
        arguments: &OverloadArguments<'_>,
        extension_receiver: Option<&hir::Expr>,
        span: Span,
    ) {
        let supplied = match arguments {
            OverloadArguments::Source(arguments) => arguments.len(),
            OverloadArguments::Lowered(arguments) => arguments.len(),
        };
        let expected = prepared[0].view.value_parameters.len();
        if prepared
            .iter()
            .all(|candidate| candidate.view.value_parameters.len() == expected)
            && expected != supplied
        {
            self.arity_diagnostic(name, prepared, expected, supplied, span);
            return;
        }

        let contextual_argument = failures
            .iter()
            .filter_map(|failure| match failure.kind {
                CandidateProbeFailureKind::Expression {
                    source_index,
                    expected: Some(_),
                    ..
                } => Some(source_index),
                _ => None,
            })
            .min();
        if let (Some(source_index), OverloadArguments::Source(expressions)) =
            (contextual_argument, arguments)
        {
            let mut details = Vec::new();
            for failure in failures.iter_mut() {
                let CandidateProbeFailureKind::Expression {
                    source_index: failed,
                    expected: Some(expected),
                    ref reason,
                    ..
                } = failure.kind
                else {
                    continue;
                };
                if failed != source_index {
                    continue;
                }
                let detail = format!("{}: {reason}", failure.state.type_name(expected));
                if !details.contains(&detail) {
                    details.push(detail);
                }
            }
            self.error(
                expressions[source_index].span(),
                format!(
                    "{} does not match any overload of `{name}`; candidate expectations: {}",
                    contextual_expr_name(&expressions[source_index]),
                    details.join("; ")
                ),
            );
            return;
        }

        if matches!(arguments, OverloadArguments::Source(_))
            && prepared.len() == 1
            && let Some(failure) = failures.first_mut()
        {
            debug_assert_eq!(failure.candidate, 0);
            let (diagnostic_span, message) = match &failure.kind {
                CandidateProbeFailureKind::Expression {
                    span: diagnostic_span,
                    reason,
                    ..
                } => (Some(*diagnostic_span), Some(reason.clone())),
                CandidateProbeFailureKind::Constraint(constraint) => (
                    Some(constraint_failure_span(
                        arguments,
                        extension_receiver,
                        constraint,
                        span,
                    )),
                    render_constraint_failure(
                        &mut failure.state,
                        &prepared[0],
                        &failure.arguments,
                        constraint,
                    ),
                ),
            };
            if let Some(message) = message {
                self.error(diagnostic_span.unwrap_or(span), message);
                return;
            }
        }

        if failures.len() == 1
            && let CandidateProbeFailureKind::Constraint(constraint) = &failures[0].kind
            && matches!(
                constraint.kind,
                ConstraintFailureKind::Kind { .. } | ConstraintFailureKind::InterfaceBound { .. }
            )
            && let Some(message) = render_constraint_failure(
                &mut failures[0].state,
                &prepared[failures[0].candidate],
                &failures[0].arguments,
                constraint,
            )
        {
            self.error(span, message);
            return;
        }

        let labels = failures
            .first_mut()
            .map(|failure| {
                argument_labels(
                    &mut failure.state,
                    &failure.arguments,
                    arguments,
                    extension_receiver.is_some(),
                )
            })
            .unwrap_or_else(|| source_argument_labels(arguments));
        let message = match extension_receiver {
            Some(receiver) => format!(
                "no overload of extension `{name}` matches receiver type {} and argument types ({})",
                self.type_name(receiver.ty),
                labels.join(", ")
            ),
            None => format!(
                "no overload of `{name}` matches argument types ({})",
                labels.join(", ")
            ),
        };
        let diagnostic_span = extension_receiver.map_or(span, |receiver| receiver.span);
        self.error(diagnostic_span, message);
    }

    fn arity_diagnostic(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        expected: usize,
        supplied: usize,
        span: Span,
    ) {
        let noun = if expected == 1 {
            "argument"
        } else {
            "arguments"
        };
        let target = if prepared.len() == 1 {
            match prepared[0].target {
                crate::call_resolution::candidates::CallableSource::Free(_) => {
                    format!("function `{name}`")
                }
                crate::call_resolution::candidates::CallableSource::Local { .. } => {
                    format!("local function `{name}`")
                }
                crate::call_resolution::candidates::CallableSource::Method(_) => {
                    format!("method `{name}`")
                }
            }
        } else {
            format!("`{name}`")
        };
        self.error(
            span,
            format!("{target} takes exactly {expected} {noun}, but {supplied} were supplied"),
        );
    }
}

fn render_constraint_failure(
    lowerer: &mut Lowerer,
    candidate: &Candidate,
    arguments: &[Option<hir::Expr>],
    failure: &ConstraintFailure,
) -> Option<String> {
    let parameter = |variable| inference_parameter(&candidate.view, variable);
    match &failure.kind {
        ConstraintFailureKind::Kind {
            variable,
            solution,
            required,
        } => {
            let required = match required {
                hir::TypeParamKind::Any => return None,
                hir::TypeParamKind::Value => "value",
                hir::TypeParamKind::Ref => "ref",
            };
            Some(format!(
                "type argument `{}` for `{}` of function `{}` must satisfy `{required}`",
                lowerer.type_name(*solution),
                parameter(*variable).name,
                lowerer.functions[candidate.function].name,
            ))
        }
        ConstraintFailureKind::InterfaceBound {
            variable,
            solution,
            required,
        } => Some(format!(
            "type argument `{}` for `{}` of function `{}` must satisfy interface upper bound `{}`",
            lowerer.type_name(*solution),
            parameter(*variable).name,
            lowerer.functions[candidate.function].name,
            lowerer.type_name(*required),
        )),
        ConstraintFailureKind::ConflictingExactBounds {
            variable,
            first,
            second,
        } => Some(format!(
            "conflicting types for `{}`: {} and {}",
            parameter(*variable).name,
            lowerer.type_name(*first),
            lowerer.type_name(*second),
        )),
        ConstraintFailureKind::NoUniqueSolution {
            variable,
            lower_bounds,
            ..
        } if lower_bounds.len() >= 2 => Some(format!(
            "conflicting types for `{}`: {} and {}",
            parameter(*variable).name,
            lowerer.type_name(lower_bounds[0]),
            lowerer.type_name(lower_bounds[1]),
        )),
        ConstraintFailureKind::NoUniqueSolution { variable, .. }
        | ConstraintFailureKind::UnresolvedTerm(TypeTerm::Variable(variable)) => Some(format!(
            "cannot infer type argument `{}` for `{}`",
            parameter(*variable).name,
            lowerer.functions[candidate.function].name,
        )),
        ConstraintFailureKind::Relation { right, .. }
            if matches!(failure.origin, ConstraintOrigin::Argument(_)) =>
        {
            let ConstraintOrigin::Argument(input) = failure.origin else {
                unreachable!()
            };
            let receiver_offset = usize::from(matches!(
                candidate.view.receiver,
                crate::call_resolution::candidates::ReceiverShape::Extension(_)
            ));
            let source_index = input.index();
            let argument = arguments.get(receiver_offset + source_index)?.as_ref()?;
            let expected = match right {
                TypeTerm::Type(ty) | TypeTerm::Rigid(ty) if !lowerer.type_contains_param(*ty) => {
                    lowerer.type_name(*ty)
                }
                _ => return None,
            };
            Some(format!(
                "argument for parameter `{}` of `{}` must be of type {expected}, found {}",
                candidate.view.value_parameters[source_index].name,
                lowerer.functions[candidate.function].name,
                lowerer.type_name(argument.ty),
            ))
        }
        _ => None,
    }
}

fn constraint_failure_span(
    arguments: &OverloadArguments<'_>,
    extension_receiver: Option<&hir::Expr>,
    failure: &ConstraintFailure,
    fallback: Span,
) -> Span {
    match failure.origin {
        ConstraintOrigin::Argument(input) => match arguments {
            OverloadArguments::Source(arguments) => arguments
                .get(input.index())
                .map_or(fallback, ast::Expr::span),
            OverloadArguments::Lowered(arguments) => arguments
                .get(input.index())
                .map_or(fallback, |arg| arg.span),
        },
        ConstraintOrigin::Receiver => extension_receiver.map_or(fallback, |receiver| receiver.span),
        _ => fallback,
    }
}

fn inference_parameter(
    view: &crate::call_resolution::candidates::CallableView,
    variable: InferenceVariableId,
) -> &hir::TypeParamDecl {
    match variable {
        InferenceVariableId::Owner(_) => &view.owner_parameters[variable.group_index()],
        InferenceVariableId::Callable(_) => &view.callable_parameters[variable.group_index()],
    }
}

fn argument_labels(
    lowerer: &mut Lowerer,
    lowered: &[Option<hir::Expr>],
    source: &OverloadArguments<'_>,
    has_receiver: bool,
) -> Vec<String> {
    let receiver_offset = usize::from(has_receiver);
    let count = match source {
        OverloadArguments::Source(arguments) => arguments.len(),
        OverloadArguments::Lowered(arguments) => arguments.len(),
    };
    (0..count)
        .map(|index| {
            lowered
                .get(receiver_offset + index)
                .and_then(Option::as_ref)
                .map(|argument| lowerer.type_name(argument.ty))
                .unwrap_or_else(|| match source {
                    OverloadArguments::Source(arguments) => contextual_expr_name(&arguments[index]),
                    OverloadArguments::Lowered(_) => "context-dependent expression".to_string(),
                })
        })
        .collect()
}

fn source_argument_labels(arguments: &OverloadArguments<'_>) -> Vec<String> {
    match arguments {
        OverloadArguments::Source(arguments) => {
            arguments.iter().map(contextual_expr_name).collect()
        }
        OverloadArguments::Lowered(arguments) => arguments
            .iter()
            .map(|_| "context-dependent expression".to_string())
            .collect(),
    }
}

fn contextual_expr_name(expr: &ast::Expr) -> String {
    match expr {
        ast::Expr::Var(name) if name.text == "None" => "None".to_string(),
        ast::Expr::ArrayLiteral { elements, .. } if elements.is_empty() => "[]".to_string(),
        ast::Expr::Lambda { .. } => "lambda".to_string(),
        ast::Expr::AnonymousFunction { .. } => "anonymous function".to_string(),
        ast::Expr::CallableReference { .. } => "callable reference".to_string(),
        _ => "context-dependent expression".to_string(),
    }
}
