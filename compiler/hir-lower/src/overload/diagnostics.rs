use super::*;

use crate::call_resolution::constraints::{ConstraintFailure, ConstraintOrigin};
use crate::call_resolution::diagnostics::{
    callable_layer_name, callable_source_signature, render_callable_constraint_failure,
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
        let views = prepared
            .iter()
            .map(|candidate| candidate.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let mut traces = Vec::with_capacity(failures.len());
        for failure in failures {
            let candidate = &prepared[failure.candidate];
            let signature = callable_source_signature(self, name, &candidate.view);
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
        let views = prepared
            .iter()
            .map(|candidate| candidate.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let traces = tied
            .iter()
            .map(|&index| {
                format!(
                    "  - {} — tied after pairwise declaration forwarding",
                    callable_source_signature(self, name, &prepared[index].view)
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
        CandidateProbeFailureKind::Constraint(constraint) => render_callable_constraint_failure(
            &failure.state,
            &candidate.view,
            &failure.arguments,
            constraint,
        ),
    }
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
