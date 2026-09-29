use super::*;

use crate::call_resolution::applicability::CallableApplicabilityInput;
use crate::call_resolution::contextual::{
    ArgumentExpression, ArgumentInferenceFailureKind, ArgumentInferenceInput, InferredArguments,
};
use crate::expr::ResolvedCallTypeArgument;

pub(super) struct ApplicableCandidate {
    pub(super) candidate: usize,
    pub(super) state: Box<Lowerer>,
    pub(super) type_args: Vec<TypeId>,
    pub(super) args: Vec<hir::Expr>,
    pub(super) argument_sinks: Vec<Vec<hir::Statement>>,
    pub(super) return_ty: TypeId,
}

pub(super) struct CandidateProbeFailure {
    pub(super) candidate: usize,
    pub(super) state: Box<Lowerer>,
    pub(super) arguments: Vec<Option<hir::Expr>>,
    pub(super) kind: CandidateProbeFailureKind,
}

pub(super) enum CandidateProbeFailureKind {
    Shape(CandidateShapeFailure),
    Intrinsic {
        span: Span,
        reason: String,
    },
    Constraint(crate::call_resolution::constraints::ConstraintFailure),
    Expression {
        source_index: usize,
        expected: Option<TypeId>,
        span: Span,
        reason: String,
    },
}

pub(super) enum CandidateShapeFailure {
    TypeArgumentArity { expected: usize, supplied: usize },
    Argument(crate::call_resolution::arguments::ArgumentShapeFailure),
}

impl Lowerer {
    pub(super) fn probe_overload_candidate(
        &self,
        candidate_index: usize,
        candidate: &Candidate,
        receiver: Option<&hir::Expr>,
        explicit_type_args: &[ResolvedCallTypeArgument],
        arguments: &OverloadArguments<'_>,
        expected_result: Option<TypeId>,
    ) -> Result<ApplicableCandidate, Box<CandidateProbeFailure>> {
        let mut state = self.clone();
        let receiver_offset = usize::from(receiver.is_some());
        let argument_map = candidate
            .argument_map
            .as_ref()
            .expect("only shape-applicable candidates are probed");
        let intrinsic_argument_expected = match arguments {
            OverloadArguments::Source(expressions) => {
                let diagnostics_before = state.diagnostics.len();
                match state.foreign_callback_argument_expected(
                    candidate.function,
                    explicit_type_args,
                    candidate
                        .argument_map
                        .as_ref()
                        .expect("only shape-applicable candidates are probed"),
                    expressions,
                    candidate.call_span,
                ) {
                    Ok(expected) => expected,
                    Err(()) => {
                        let reason = diagnostic_reason(&state, diagnostics_before);
                        let span = diagnostic_span(&state, diagnostics_before, candidate.call_span);
                        return Err(Box::new(CandidateProbeFailure {
                            candidate: candidate_index,
                            state: Box::new(state),
                            arguments: receiver.cloned().map(Some).into_iter().collect(),
                            kind: CandidateProbeFailureKind::Intrinsic { span, reason },
                        }));
                    }
                }
            }
            OverloadArguments::Lowered(_) => None,
        };

        let expressions = match arguments {
            OverloadArguments::Source(expressions) => expressions
                .iter()
                .map(|argument| ArgumentExpression::Source(&argument.expression))
                .collect::<Vec<_>>(),
            OverloadArguments::Lowered(arguments) => arguments
                .iter()
                .map(ArgumentExpression::Lowered)
                .collect::<Vec<_>>(),
        };
        let patterns = argument_map.inference_patterns(&candidate.view.value_parameters);
        let parameters = candidate
            .view
            .owner_parameters
            .iter()
            .chain(&candidate.view.callable_parameters)
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let (mut session, environment) =
            state.callable_applicability_session(CallableApplicabilityInput {
                view: &candidate.view,
                owner_arguments: &candidate.owner_arguments,
                explicit_arguments: explicit_type_args,
                receiver_type: receiver.map(|receiver| receiver.ty),
            });
        let InferredArguments {
            types,
            values,
            sinks: argument_sinks,
        } = match state.infer_contextual_arguments(ArgumentInferenceInput {
            expressions: &expressions,
            patterns: &patterns,
            parameters: &parameters,
            session: &mut session,
            environment,
            expected_result: expected_result.map(|expected| (candidate.view.return_type, expected)),
            forced_hint: intrinsic_argument_expected,
        }) {
            Ok(arguments) => arguments,
            Err(failure) => {
                let kind = match failure.kind {
                    ArgumentInferenceFailureKind::Constraint(failure) => {
                        CandidateProbeFailureKind::Constraint(failure)
                    }
                    ArgumentInferenceFailureKind::Expression(failure) => {
                        CandidateProbeFailureKind::Expression {
                            source_index: failure.source_index,
                            expected: failure.expected,
                            span: failure.span,
                            reason: failure.reason,
                        }
                    }
                };
                return Err(Box::new(CandidateProbeFailure {
                    candidate: candidate_index,
                    state: Box::new(state),
                    arguments: receiver
                        .cloned()
                        .map(Some)
                        .into_iter()
                        .chain(failure.arguments)
                        .collect(),
                    kind,
                }));
            }
        };
        let type_args = types
            .owner
            .into_iter()
            .chain(types.callable)
            .collect::<Vec<_>>();
        let lowered = receiver
            .cloned()
            .into_iter()
            .chain(values)
            .collect::<Vec<_>>();

        let mut args = Vec::with_capacity(lowered.len());
        for (index, argument) in lowered.into_iter().enumerate() {
            let source_index = index.saturating_sub(receiver_offset);
            let expected = (receiver_offset == 0)
                .then_some(intrinsic_argument_expected)
                .flatten()
                .and_then(|(expected_source, expected)| {
                    (source_index == expected_source).then_some(expected)
                })
                .unwrap_or_else(|| {
                    state.substitute_call_level(candidate.params[index], &type_args)
                });
            if !state.is_subtype(argument.ty, expected) {
                let reason = format!(
                    "expression has type {}, expected {}",
                    state.type_name(argument.ty),
                    state.type_name(expected)
                );
                return Err(Box::new(CandidateProbeFailure {
                    candidate: candidate_index,
                    state: Box::new(state),
                    arguments: args.into_iter().map(Some).collect(),
                    kind: CandidateProbeFailureKind::Expression {
                        source_index,
                        expected: Some(expected),
                        span: argument.span,
                        reason,
                    },
                }));
            }
            args.push(state.adapt_to(argument, expected));
        }
        let return_ty = state.substitute_call_level(candidate.return_ty, &type_args);
        Ok(ApplicableCandidate {
            candidate: candidate_index,
            state: Box::new(state),
            type_args,
            args,
            argument_sinks,
            return_ty,
        })
    }
}

fn diagnostic_reason(lowerer: &Lowerer, diagnostics_before: usize) -> String {
    let messages = lowerer.diagnostics[diagnostics_before..]
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    if messages.is_empty() {
        "expression could not be typed".to_string()
    } else {
        messages.join(", ")
    }
}

fn diagnostic_span(lowerer: &Lowerer, diagnostics_before: usize, fallback: Span) -> Span {
    lowerer.diagnostics[diagnostics_before..]
        .first()
        .and_then(|diagnostic| diagnostic.span)
        .unwrap_or(fallback)
}
