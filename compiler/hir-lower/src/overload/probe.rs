use super::*;

use crate::call_resolution::applicability::DeclarationTypeArguments;
use crate::call_resolution::contextual::{ArgumentExpression, ArgumentInferenceFailureKind};
use crate::call_resolution::probe::{CallInferenceInput, InferredCall};
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
        context_dependent: bool,
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
        let shape = if !candidate.explicit_arity_match {
            Some(CandidateShapeFailure::TypeArgumentArity {
                expected: candidate.own_type_param_count,
                supplied: explicit_type_args.len(),
            })
        } else {
            candidate
                .argument_map
                .as_ref()
                .err()
                .cloned()
                .map(CandidateShapeFailure::Argument)
        };
        if let Some(shape) = shape {
            return Err(Box::new(CandidateProbeFailure {
                candidate: candidate_index,
                state: Box::new(state),
                arguments: Vec::new(),
                kind: CandidateProbeFailureKind::Shape(shape),
            }));
        }
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
        let bound_receiver = receiver.map(|receiver| {
            let crate::call_resolution::candidates::ReceiverShape::Extension(expected) =
                candidate.view.receiver
            else {
                unreachable!("direct call applicability binds only extension receivers")
            };
            (expected, receiver.ty)
        });
        let InferredCall {
            types,
            bindings,
            mut values,
            sinks: argument_sinks,
            return_type: return_ty,
            ..
        } = match state.infer_call_arguments(CallInferenceInput {
            signature: &candidate.view.signature,
            argument_map,
            type_arguments: DeclarationTypeArguments::Callable {
                owner_arguments: &candidate.owner_arguments,
            },
            explicit_arguments: explicit_type_args,
            bound_receiver,
            expressions: &expressions,
            expected_result,
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
                            context_dependent: failure.context_dependent,
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
        if let Some(receiver) = receiver {
            let expected = state.instantiate_method_ty(
                bound_receiver
                    .expect("an extension receiver has a declaration type")
                    .0,
                &bindings,
            );
            values.insert(0, state.adapt_to(receiver.clone(), expected));
        }
        Ok(ApplicableCandidate {
            candidate: candidate_index,
            state: Box::new(state),
            type_args,
            args: values,
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
