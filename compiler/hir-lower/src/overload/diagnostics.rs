use super::*;

use crate::call_resolution::constraints::{ConstraintFailure, ConstraintOrigin};
use crate::call_resolution::diagnostics::{
    callable_layer_name, callable_source_signature, render_callable_constraint_failure,
};
use crate::expr::ResolvedCallTypeArgument;
use crate::overload::probe::{
    ApplicableCandidate, CandidateProbeFailure, CandidateProbeFailureKind, CandidateShapeFailure,
};

mod integers;
use integers::{primitive_integer_conversion_suggestion, render_literal_exact_commits};

pub(super) struct CandidateFailureContext<'call, 'arguments> {
    pub(super) arguments: &'call OverloadArguments<'arguments>,
    pub(super) extension_receiver: Option<&'call hir::Expr>,
    pub(super) explicit_type_args: &'call [ResolvedCallTypeArgument],
    pub(super) span: Span,
}

pub(super) struct AmbiguityContext<'call, 'arguments> {
    pub(super) applicable: &'call [ApplicableCandidate],
    pub(super) arguments: &'call OverloadArguments<'arguments>,
    pub(super) receiver_offset: usize,
    pub(super) span: Span,
}

impl Lowerer {
    pub(super) fn candidate_failures_diagnostic(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        failures: &mut [CandidateProbeFailure],
        context: CandidateFailureContext<'_, '_>,
    ) {
        let CandidateFailureContext {
            arguments,
            extension_receiver,
            explicit_type_args,
            span,
        } = context;
        debug_assert_eq!(failures.len(), prepared.len());
        failures.sort_by_key(|failure| failure.candidate);
        if failures
            .iter()
            .all(|failure| matches!(failure.kind, CandidateProbeFailureKind::Expression { .. }))
            && failures
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
            let same_diagnostics = !diagnostics.is_empty()
                && failures[1..]
                    .iter()
                    .all(|failure| failure.state.diagnostics[baseline..] == diagnostics);
            let independent_of_context = failures.iter().all(|failure| {
                matches!(
                    failure.kind,
                    CandidateProbeFailureKind::Expression {
                        context_dependent: false,
                        ..
                    }
                )
            });
            if same_diagnostics && independent_of_context {
                self.diagnostics.extend(diagnostics);
                return;
            }
        }
        let diagnostic_span = common_failure_span(
            failures,
            arguments,
            extension_receiver,
            explicit_type_args,
            span,
        );
        let views = prepared
            .iter()
            .map(|candidate| candidate.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let mut traces = Vec::with_capacity(failures.len());
        for failure in failures {
            let candidate = &prepared[failure.candidate];
            let signature = callable_source_signature(self, name, &candidate.view);
            let reason =
                render_candidate_failure(candidate, failure, explicit_type_args, arguments);
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
        context: AmbiguityContext<'_, '_>,
    ) {
        let AmbiguityContext {
            applicable,
            arguments,
            receiver_offset,
            span,
        } = context;
        let views = prepared
            .iter()
            .map(|candidate| candidate.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let traces = tied
            .iter()
            .map(|&index| {
                let candidate = &prepared[index];
                let transaction = applicable
                    .iter()
                    .find(|transaction| transaction.candidate == index)
                    .expect("every tied candidate has an applicability transaction");
                let literal_commits = render_literal_exact_commits(
                    candidate,
                    transaction,
                    arguments,
                    receiver_offset,
                );
                format!(
                    "  - {} — tied after pairwise declaration forwarding{}",
                    callable_source_signature(self, name, &candidate.view),
                    literal_commits.map_or_else(String::new, |commits| format!(
                        "; requires integer literal exact {}: {commits}",
                        if commits.contains(", ") {
                            "commits"
                        } else {
                            "commit"
                        }
                    ))
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            span,
            format!("call to `{name}` is ambiguous in {layer} layer:\n{traces}"),
        );
    }
}

fn render_candidate_failure(
    candidate: &Candidate,
    failure: &CandidateProbeFailure,
    explicit_type_args: &[ResolvedCallTypeArgument],
    arguments: &OverloadArguments<'_>,
) -> String {
    let mut reason = match &failure.kind {
        CandidateProbeFailureKind::Shape(CandidateShapeFailure::TypeArgumentArity {
            expected,
            supplied,
        }) => format!("expects {expected} explicit type argument(s), but {supplied} were supplied"),
        CandidateProbeFailureKind::Shape(CandidateShapeFailure::Argument(failure)) => {
            failure.describe()
        }
        CandidateProbeFailureKind::Expression {
            source_index,
            expected,
            reason,
            ..
        } => {
            let parameter = candidate
                .argument_map
                .as_ref()
                .ok()
                .map(|mapping| {
                    let (parameter, _) = mapping.source_binding(
                        crate::call_resolution::arguments::SourceInputId::from_index(*source_index),
                    );
                    parameter.index()
                })
                .and_then(|index| candidate.view.value_parameters.get(index))
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
        CandidateProbeFailureKind::Intrinsic { reason, .. } => reason.clone(),
        CandidateProbeFailureKind::Constraint(constraint) => render_callable_constraint_failure(
            &failure.state,
            &candidate.view,
            candidate.argument_map.as_ref().ok(),
            &failure.arguments,
            constraint,
        ),
    };
    if let Some(suggestion) = primitive_integer_conversion_suggestion(candidate, failure, arguments)
    {
        reason.push_str(&suggestion);
    }
    let CandidateProbeFailureKind::Constraint(constraint) = &failure.kind else {
        return reason;
    };
    let Some(crate::call_resolution::constraints::InferenceVariableId::Callable(variable)) =
        constraint.kind.inference_variable()
    else {
        return reason;
    };
    let failed =
        crate::call_resolution::constraints::InferenceVariableId::Callable(variable).group_index();
    if !matches!(
        explicit_type_args.get(failed),
        Some(ResolvedCallTypeArgument::Infer { .. })
    ) {
        return reason;
    }
    let fixed = explicit_type_args
        .iter()
        .zip(&candidate.view.callable_parameters)
        .filter_map(|(argument, parameter)| {
            let ResolvedCallTypeArgument::Explicit { ty, .. } = argument else {
                return None;
            };
            Some(format!(
                "{} = {}",
                parameter.name,
                failure.state.type_name(*ty)
            ))
        })
        .collect::<Vec<_>>();
    if fixed.is_empty() {
        reason
    } else {
        format!("{reason}; fixed type arguments: {}", fixed.join(", "))
    }
}

fn common_failure_span(
    failures: &[CandidateProbeFailure],
    arguments: &OverloadArguments<'_>,
    extension_receiver: Option<&hir::Expr>,
    explicit_type_args: &[ResolvedCallTypeArgument],
    fallback: Span,
) -> Span {
    let Some(first) = failures.first() else {
        return fallback;
    };
    let first = candidate_failure_span(
        first,
        arguments,
        extension_receiver,
        explicit_type_args,
        fallback,
    );
    if failures[1..].iter().all(|failure| {
        candidate_failure_span(
            failure,
            arguments,
            extension_receiver,
            explicit_type_args,
            fallback,
        ) == first
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
    explicit_type_args: &[ResolvedCallTypeArgument],
    fallback: Span,
) -> Span {
    match &failure.kind {
        CandidateProbeFailureKind::Shape(_) => fallback,
        CandidateProbeFailureKind::Intrinsic { span, .. } => *span,
        CandidateProbeFailureKind::Expression { span, .. } => *span,
        CandidateProbeFailureKind::Constraint(failure) => constraint_failure_span(
            arguments,
            extension_receiver,
            explicit_type_args,
            failure,
            fallback,
        ),
    }
}

fn constraint_failure_span(
    arguments: &OverloadArguments<'_>,
    extension_receiver: Option<&hir::Expr>,
    explicit_type_args: &[ResolvedCallTypeArgument],
    failure: &ConstraintFailure,
    fallback: Span,
) -> Span {
    if let Some(crate::call_resolution::constraints::InferenceVariableId::Callable(variable)) =
        failure.kind.inference_variable()
        && let Some(argument) = explicit_type_args.get(
            crate::call_resolution::constraints::InferenceVariableId::Callable(variable)
                .group_index(),
        )
    {
        return argument.span();
    }
    match failure.origin {
        ConstraintOrigin::Argument(input) => match arguments {
            OverloadArguments::Source(arguments) => arguments
                .get(input.index())
                .map_or(fallback, ast::CallArgument::span),
            OverloadArguments::Lowered(arguments) => arguments
                .get(input.index())
                .map_or(fallback, |arg| arg.span),
        },
        ConstraintOrigin::Receiver => extension_receiver.map_or(fallback, |receiver| receiver.span),
        _ => fallback,
    }
}
