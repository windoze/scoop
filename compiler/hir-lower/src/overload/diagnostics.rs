use super::*;

use crate::call_resolution::constraints::{
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin, InferenceVariableId, TypeTerm,
};
use crate::overload::probe::{
    CandidateProbeFailure, CandidateProbeFailureKind, CandidateShapeFailure,
};

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
        debug_assert_eq!(failures.len(), prepared.len());
        failures.sort_by_key(|failure| failure.candidate);
        if failures.iter().all(|failure| {
            matches!(
                failure.kind,
                CandidateProbeFailureKind::Expression { expected: None, .. }
            )
        }) && failures
            .windows(2)
            .all(|pair| match (&pair[0].kind, &pair[1].kind) {
                (
                    CandidateProbeFailureKind::Expression {
                        span: left_span,
                        reason: left_reason,
                        ..
                    },
                    CandidateProbeFailureKind::Expression {
                        span: right_span,
                        reason: right_reason,
                        ..
                    },
                ) => left_span == right_span && left_reason == right_reason,
                _ => false,
            })
        {
            let baseline = self.diagnostics.len();
            let diagnostics = failures[0].state.diagnostics[baseline..].to_vec();
            if !diagnostics.is_empty() {
                self.diagnostics.extend(diagnostics);
                return;
            }
        }
        let diagnostic_span = common_failure_span(failures, arguments, extension_receiver, span);
        let layer = self.candidate_layer_name(prepared, extension_receiver.is_some());
        let mut traces = Vec::with_capacity(failures.len());
        for failure in failures {
            let candidate = &prepared[failure.candidate];
            let signature = self.candidate_source_signature(name, candidate);
            let reason = render_candidate_failure(candidate, failure);
            traces.push(format!("  - {signature} — {reason}"));
        }
        self.error(
            diagnostic_span,
            format!(
                "no applicable candidate for `{name}` in {layer} layer:\n{}",
                traces.join("\n")
            ),
        );
    }

    pub(super) fn ambiguity_diagnostic(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        tied: &[usize],
        span: Span,
    ) {
        let is_extension = prepared.iter().any(|candidate| {
            matches!(
                candidate.view.receiver,
                crate::call_resolution::candidates::ReceiverShape::Extension(_)
            )
        });
        let layer = self.candidate_layer_name(prepared, is_extension);
        let traces = tied
            .iter()
            .map(|&index| {
                format!(
                    "  - {} — tied after pairwise declaration forwarding",
                    self.candidate_source_signature(name, &prepared[index])
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            span,
            format!("call to `{name}` is ambiguous in {layer} layer:\n{traces}"),
        );
    }

    fn candidate_layer_name(&self, prepared: &[Candidate], is_extension: bool) -> &'static str {
        if is_extension {
            return "extension candidate";
        }
        match prepared[0].target {
            crate::call_resolution::candidates::CallableSource::Local { .. } => {
                "lexical local candidate"
            }
            crate::call_resolution::candidates::CallableSource::Method(_) => "member candidate",
            crate::call_resolution::candidates::CallableSource::Free(function) => {
                let call_site_is_core = self.current_file < self.user_file_index;
                let candidate_is_core = self.function_files[&function] < self.user_file_index;
                if call_site_is_core == candidate_is_core {
                    "current-unit top-level candidate"
                } else {
                    "implicit-import candidate"
                }
            }
        }
    }

    fn candidate_source_signature(&self, name: &str, candidate: &Candidate) -> String {
        let view = &candidate.view;
        let mut all_parameters = view.owner_parameters.clone();
        all_parameters.extend(view.callable_parameters.iter().cloned());
        let callable_parameters =
            render_type_parameters(self, &view.callable_parameters, &all_parameters);
        let declared_name = match view.target {
            crate::call_resolution::candidates::CallableSource::Method(_) => {
                let function_name = &self.functions[candidate.function].name;
                let (owner, _) = function_name
                    .rsplit_once('.')
                    .unwrap_or((function_name.as_str(), name));
                let owner_parameters =
                    render_type_parameters(self, &view.owner_parameters, &all_parameters);
                format!("{owner}{owner_parameters}.{name}{callable_parameters}")
            }
            crate::call_resolution::candidates::CallableSource::Free(_)
                if let crate::call_resolution::candidates::ReceiverShape::Extension(receiver) =
                    view.receiver =>
            {
                format!(
                    "{}.{name}{callable_parameters}",
                    self.type_name_with_params(receiver, &all_parameters)
                )
            }
            crate::call_resolution::candidates::CallableSource::Free(_)
            | crate::call_resolution::candidates::CallableSource::Local { .. } => {
                format!("{name}{callable_parameters}")
            }
        };
        let parameters = view
            .value_parameters
            .iter()
            .map(|parameter| {
                format!(
                    "{}: {}",
                    parameter.name,
                    self.type_name_with_params(parameter.ty, &all_parameters)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let return_type = self.type_name_with_params(view.return_type, &all_parameters);
        let suspend = if view.effects.is_suspend {
            "suspend "
        } else {
            ""
        };
        format!("{suspend}fun {declared_name}({parameters}): {return_type}")
    }
}

fn render_candidate_failure(candidate: &Candidate, failure: &CandidateProbeFailure) -> String {
    match &failure.kind {
        CandidateProbeFailureKind::Shape(CandidateShapeFailure::TypeArgumentArity {
            expected,
            supplied,
        }) => format!("expects {expected} explicit type argument(s), but {supplied} were supplied"),
        CandidateProbeFailureKind::Shape(CandidateShapeFailure::ArgumentArity {
            expected,
            supplied,
        }) => format!("expects {expected} argument(s), but {supplied} were supplied"),
        CandidateProbeFailureKind::Expression {
            source_index,
            expected,
            reason,
            ..
        } => {
            let parameter = candidate
                .view
                .value_parameters
                .get(*source_index)
                .map(|parameter| format!("argument for `{}`", parameter.name))
                .unwrap_or_else(|| format!("argument {}", source_index + 1));
            match expected {
                Some(expected) => format!(
                    "{parameter} (expected {}): {reason}",
                    failure.state.type_name(*expected)
                ),
                None => format!("{parameter}: {reason}"),
            }
        }
        CandidateProbeFailureKind::Constraint(constraint) => {
            render_constraint_failure(&failure.state, candidate, &failure.arguments, constraint)
        }
    }
}

fn render_constraint_failure(
    lowerer: &Lowerer,
    candidate: &Candidate,
    arguments: &[Option<hir::Expr>],
    failure: &ConstraintFailure,
) -> String {
    let parameter = |variable| inference_parameter(&candidate.view, variable);
    match &failure.kind {
        ConstraintFailureKind::Kind {
            variable,
            solution,
            required,
        } => {
            let required = match required {
                hir::TypeParamKind::Any => "any",
                hir::TypeParamKind::Value => "value",
                hir::TypeParamKind::Ref => "ref",
            };
            format!(
                "type argument `{}` for `{}` must satisfy `{required}`",
                lowerer.type_name(*solution),
                parameter(*variable).name,
            )
        }
        ConstraintFailureKind::InterfaceBound {
            variable,
            solution,
            required,
        } => format!(
            "type argument `{}` for `{}` must satisfy interface upper bound `{}`",
            lowerer.type_name(*solution),
            parameter(*variable).name,
            lowerer.type_name(*required),
        ),
        ConstraintFailureKind::ConflictingExactBounds {
            variable,
            first,
            second,
        } => format!(
            "conflicting types for `{}`: {} and {}",
            parameter(*variable).name,
            lowerer.type_name(*first),
            lowerer.type_name(*second),
        ),
        ConstraintFailureKind::NoUniqueSolution {
            variable,
            lower_bounds,
            ..
        } if lower_bounds.len() >= 2 => {
            format!(
                "conflicting types for `{}`: {} and {}",
                parameter(*variable).name,
                lowerer.type_name(lower_bounds[0]),
                lowerer.type_name(lower_bounds[1]),
            )
        }
        ConstraintFailureKind::NoUniqueSolution { variable, .. } => {
            format!(
                "cannot infer a unique type argument for `{}`",
                parameter(*variable).name
            )
        }
        ConstraintFailureKind::UnresolvedTerm(TypeTerm::Variable(variable)) => {
            format!("cannot infer type argument `{}`", parameter(*variable).name)
        }
        ConstraintFailureKind::Relation {
            relation,
            left,
            right,
        } if matches!(failure.origin, ConstraintOrigin::Argument(_)) => {
            let ConstraintOrigin::Argument(input) = failure.origin else {
                unreachable!()
            };
            let receiver_offset = usize::from(matches!(
                candidate.view.receiver,
                crate::call_resolution::candidates::ReceiverShape::Extension(_)
            ));
            let source_index = input.index();
            let found = arguments
                .get(receiver_offset + source_index)
                .and_then(Option::as_ref)
                .map(|argument| lowerer.type_name(argument.ty))
                .unwrap_or_else(|| render_type_term(lowerer, candidate, *left));
            format!(
                "argument for `{}` has type {found}, which {} {}",
                candidate.view.value_parameters[source_index].name,
                relation_failure_phrase(*relation),
                render_type_term(lowerer, candidate, *right),
            )
        }
        ConstraintFailureKind::Relation {
            relation,
            left,
            right,
        } => format!(
            "{} {} {}",
            render_type_term(lowerer, candidate, *left),
            relation_failure_phrase(*relation),
            render_type_term(lowerer, candidate, *right),
        ),
        ConstraintFailureKind::CallableShape(mismatch) => match mismatch {
            crate::call_resolution::constraints::CallableShapeMismatch::ExpectedCallable => {
                "expected a callable type".to_string()
            }
            crate::call_resolution::constraints::CallableShapeMismatch::Suspend => {
                "ordinary and suspend callable shapes differ".to_string()
            }
            crate::call_resolution::constraints::CallableShapeMismatch::Arity {
                expected,
                actual,
            } => format!(
                "callable shape expects {expected} parameter(s), but the actual type has {actual}"
            ),
        },
        ConstraintFailureKind::ForeignVariable(_) => {
            "candidate references an inference variable from another session".to_string()
        }
        ConstraintFailureKind::ForeignTypeParameter(_) => {
            "candidate references a type parameter outside its declaration".to_string()
        }
        ConstraintFailureKind::UnresolvedTerm(term) => format!(
            "cannot resolve type term {}",
            render_type_term(lowerer, candidate, *term)
        ),
        ConstraintFailureKind::NonConcreteApplication(_) => {
            "candidate result is not a complete concrete type application".to_string()
        }
    }
}

fn relation_failure_phrase(
    relation: crate::call_resolution::constraints::RelationKind,
) -> &'static str {
    match relation {
        crate::call_resolution::constraints::RelationKind::Equal => "is not equal to",
        crate::call_resolution::constraints::RelationKind::Subtype => "is not a subtype of",
    }
}

fn render_type_term(lowerer: &Lowerer, candidate: &Candidate, term: TypeTerm) -> String {
    match term {
        TypeTerm::Type(ty) | TypeTerm::Rigid(ty) => {
            let mut parameters = candidate.view.owner_parameters.clone();
            parameters.extend(candidate.view.callable_parameters.iter().cloned());
            lowerer.type_name_with_params(ty, &parameters)
        }
        TypeTerm::Variable(variable) => inference_parameter(&candidate.view, variable).name.clone(),
    }
}

fn render_type_parameters(
    lowerer: &Lowerer,
    parameters: &[hir::TypeParamDecl],
    all_parameters: &[hir::TypeParamDecl],
) -> String {
    if parameters.is_empty() {
        return String::new();
    }
    let parameters = parameters
        .iter()
        .map(|parameter| {
            let variance = match parameter.variance {
                hir::Variance::Invariant => "",
                hir::Variance::In => "in ",
                hir::Variance::Out => "out ",
            };
            let bound = match &parameter.bounds {
                hir::TypeParamBounds::Unconstrained => String::new(),
                hir::TypeParamBounds::Value { .. } => " : value".to_string(),
                hir::TypeParamBounds::Ref { .. } => " : ref".to_string(),
                hir::TypeParamBounds::Interfaces(bounds) => format!(
                    " : {}",
                    bounds
                        .iter()
                        .map(|bound| {
                            lowerer.type_name_with_params(
                                lowerer.interface_applications[bound.application].canonical_type,
                                all_parameters,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" & ")
                ),
            };
            format!("{variance}{}{bound}", parameter.name)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("<{parameters}>")
}

fn common_failure_span(
    failures: &[CandidateProbeFailure],
    arguments: &OverloadArguments<'_>,
    extension_receiver: Option<&hir::Expr>,
    fallback: Span,
) -> Span {
    let Some(first) = failures.first() else {
        return fallback;
    };
    let first = candidate_failure_span(first, arguments, extension_receiver, fallback);
    if failures[1..].iter().all(|failure| {
        candidate_failure_span(failure, arguments, extension_receiver, fallback) == first
    }) {
        first
    } else {
        fallback
    }
}

fn candidate_failure_span(
    failure: &CandidateProbeFailure,
    arguments: &OverloadArguments<'_>,
    extension_receiver: Option<&hir::Expr>,
    fallback: Span,
) -> Span {
    match &failure.kind {
        CandidateProbeFailureKind::Shape(_) => fallback,
        CandidateProbeFailureKind::Expression { span, .. } => *span,
        CandidateProbeFailureKind::Constraint(failure) => {
            constraint_failure_span(arguments, extension_receiver, failure, fallback)
        }
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
