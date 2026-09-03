use super::*;

use crate::call_resolution::applicability::CallableApplicabilityInput;

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
    Constraint(crate::call_resolution::constraints::ConstraintFailure),
    Expression {
        source_index: usize,
        expected: Option<TypeId>,
        span: Span,
        reason: String,
    },
}

impl Lowerer {
    pub(super) fn probe_overload_candidate(
        &self,
        candidate_index: usize,
        candidate: &Candidate,
        receiver: Option<&hir::Expr>,
        explicit_type_args: &[TypeId],
        arguments: &OverloadArguments<'_>,
        expected_result: Option<TypeId>,
    ) -> Result<ApplicableCandidate, Box<CandidateProbeFailure>> {
        let mut state = self.clone();
        let receiver_offset = usize::from(receiver.is_some());
        let source_count = match arguments {
            OverloadArguments::Source(expressions) => expressions.len(),
            OverloadArguments::Lowered(arguments) => arguments.len(),
        };
        let mut lowered = Vec::with_capacity(receiver_offset + source_count);
        if let Some(receiver) = receiver {
            lowered.push(Some(receiver.clone()));
        }
        let mut argument_sinks = (0..source_count).map(|_| Vec::new()).collect::<Vec<_>>();

        match arguments {
            OverloadArguments::Source(expressions) => {
                lowered.extend((0..expressions.len()).map(|_| None));
                for input in &candidate
                    .argument_map
                    .as_ref()
                    .expect("only shape-applicable candidates are probed")
                    .source_order
                {
                    let source_index = input.index();
                    if state.expr_requires_expected_type(&expressions[source_index]) {
                        continue;
                    }
                    let diagnostics_before = state.diagnostics.len();
                    let argument = state.lower_expr(
                        &expressions[source_index],
                        &mut argument_sinks[source_index],
                        None,
                    );
                    if let Some(argument) = argument
                        && state.diagnostics.len() == diagnostics_before
                    {
                        lowered[receiver_offset + source_index] = Some(argument);
                        continue;
                    }
                    let reason = diagnostic_reason(&state, diagnostics_before);
                    let span = diagnostic_span(
                        &state,
                        diagnostics_before,
                        expressions[source_index].span(),
                    );
                    return Err(Box::new(CandidateProbeFailure {
                        candidate: candidate_index,
                        state: Box::new(state),
                        arguments: lowered,
                        kind: CandidateProbeFailureKind::Expression {
                            source_index,
                            expected: None,
                            span,
                            reason,
                        },
                    }));
                }
            }
            OverloadArguments::Lowered(arguments) => {
                lowered.extend(arguments.iter().cloned().map(Some));
            }
        }

        let mut use_expected_result = false;
        let provisional_type_args = loop {
            let argument_types = argument_types(&lowered, receiver_offset);
            let input = CallableApplicabilityInput {
                view: &candidate.view,
                argument_map: candidate
                    .argument_map
                    .as_ref()
                    .expect("only shape-applicable candidates are probed"),
                owner_arguments: &candidate.owner_arguments,
                explicit_arguments: explicit_type_args,
                receiver_type: receiver.map(|receiver| receiver.ty),
                argument_types: &argument_types,
                expected_result: use_expected_result.then_some(expected_result).flatten(),
            };
            let partial = match state.partially_solve_callable_applicability(input) {
                Ok(partial) => partial,
                Err(failure) => {
                    return Err(Box::new(CandidateProbeFailure {
                        candidate: candidate_index,
                        state: Box::new(state),
                        arguments: lowered,
                        kind: CandidateProbeFailureKind::Constraint(failure),
                    }));
                }
            };
            if partial.iter().all(Option::is_some) {
                break partial.into_iter().flatten().collect::<Vec<_>>();
            }

            let mut progress = false;
            if let OverloadArguments::Source(expressions) = arguments {
                for source_index in 0..expressions.len() {
                    let parameter_index = receiver_offset + source_index;
                    if lowered[parameter_index].is_some() {
                        continue;
                    }
                    let Some(expected) =
                        state.try_substitute(candidate.params[parameter_index], &partial)
                    else {
                        continue;
                    };
                    let diagnostics_before = state.diagnostics.len();
                    let argument = state.lower_expr(
                        &expressions[source_index],
                        &mut argument_sinks[source_index],
                        Some(expected),
                    );
                    let Some(argument) = argument else {
                        let reason = diagnostic_reason(&state, diagnostics_before);
                        let span = diagnostic_span(
                            &state,
                            diagnostics_before,
                            expressions[source_index].span(),
                        );
                        return Err(Box::new(CandidateProbeFailure {
                            candidate: candidate_index,
                            state: Box::new(state),
                            arguments: lowered,
                            kind: CandidateProbeFailureKind::Expression {
                                source_index,
                                expected: Some(expected),
                                span,
                                reason,
                            },
                        }));
                    };
                    if state.diagnostics.len() != diagnostics_before {
                        let reason = diagnostic_reason(&state, diagnostics_before);
                        let span = diagnostic_span(
                            &state,
                            diagnostics_before,
                            expressions[source_index].span(),
                        );
                        return Err(Box::new(CandidateProbeFailure {
                            candidate: candidate_index,
                            state: Box::new(state),
                            arguments: lowered,
                            kind: CandidateProbeFailureKind::Expression {
                                source_index,
                                expected: Some(expected),
                                span,
                                reason,
                            },
                        }));
                    }
                    lowered[parameter_index] = Some(argument);
                    progress = true;
                }
            }
            if progress {
                continue;
            }
            if !use_expected_result && expected_result.is_some() {
                use_expected_result = true;
                continue;
            }

            let Some(expressions) = (match arguments {
                OverloadArguments::Source(expressions) => Some(*expressions),
                OverloadArguments::Lowered(_) => None,
            }) else {
                let failure = state
                    .solve_callable_applicability(input)
                    .expect_err("an incomplete lowered candidate has no complete solution");
                return Err(Box::new(CandidateProbeFailure {
                    candidate: candidate_index,
                    state: Box::new(state),
                    arguments: lowered,
                    kind: CandidateProbeFailureKind::Constraint(failure),
                }));
            };

            let mut first_failure = None;
            for source_index in 0..expressions.len() {
                let parameter_index = receiver_offset + source_index;
                if lowered[parameter_index].is_some() {
                    continue;
                }
                let mut attempt = state.clone();
                let diagnostics_before = attempt.diagnostics.len();
                let mut argument_sink = Vec::new();
                let argument =
                    attempt.lower_expr(&expressions[source_index], &mut argument_sink, None);
                if let Some(argument) = argument
                    && attempt.diagnostics.len() == diagnostics_before
                {
                    state = attempt;
                    lowered[parameter_index] = Some(argument);
                    argument_sinks[source_index] = argument_sink;
                    progress = true;
                    break;
                }
                first_failure.get_or_insert_with(|| {
                    (
                        source_index,
                        diagnostic_span(
                            &attempt,
                            diagnostics_before,
                            expressions[source_index].span(),
                        ),
                        diagnostic_reason(&attempt, diagnostics_before),
                    )
                });
            }
            if progress {
                continue;
            }
            let Some((source_index, span, reason)) = first_failure else {
                let failure = state
                    .solve_callable_applicability(input)
                    .expect_err("an incomplete fully typed candidate has no complete solution");
                return Err(Box::new(CandidateProbeFailure {
                    candidate: candidate_index,
                    state: Box::new(state),
                    arguments: lowered,
                    kind: CandidateProbeFailureKind::Constraint(failure),
                }));
            };
            return Err(Box::new(CandidateProbeFailure {
                candidate: candidate_index,
                state: Box::new(state),
                arguments: lowered,
                kind: CandidateProbeFailureKind::Expression {
                    source_index,
                    expected: None,
                    span,
                    reason,
                },
            }));
        };

        if let OverloadArguments::Source(expressions) = arguments {
            for source_index in 0..expressions.len() {
                let parameter_index = receiver_offset + source_index;
                if lowered[parameter_index].is_some() {
                    continue;
                }
                let expected = state.substitute_call_level(
                    candidate.params[parameter_index],
                    &provisional_type_args,
                );
                let diagnostics_before = state.diagnostics.len();
                let argument = state.lower_expr(
                    &expressions[source_index],
                    &mut argument_sinks[source_index],
                    Some(expected),
                );
                let Some(argument) = argument else {
                    let reason = diagnostic_reason(&state, diagnostics_before);
                    let span = diagnostic_span(
                        &state,
                        diagnostics_before,
                        expressions[source_index].span(),
                    );
                    return Err(Box::new(CandidateProbeFailure {
                        candidate: candidate_index,
                        state: Box::new(state),
                        arguments: lowered,
                        kind: CandidateProbeFailureKind::Expression {
                            source_index,
                            expected: Some(expected),
                            span,
                            reason,
                        },
                    }));
                };
                if state.diagnostics.len() != diagnostics_before {
                    let reason = diagnostic_reason(&state, diagnostics_before);
                    let span = diagnostic_span(
                        &state,
                        diagnostics_before,
                        expressions[source_index].span(),
                    );
                    return Err(Box::new(CandidateProbeFailure {
                        candidate: candidate_index,
                        state: Box::new(state),
                        arguments: lowered,
                        kind: CandidateProbeFailureKind::Expression {
                            source_index,
                            expected: Some(expected),
                            span,
                            reason,
                        },
                    }));
                }
                lowered[parameter_index] = Some(argument);
            }
        }

        let argument_types = argument_types(&lowered, receiver_offset);
        let type_args = match state.solve_callable_applicability(CallableApplicabilityInput {
            view: &candidate.view,
            argument_map: candidate
                .argument_map
                .as_ref()
                .expect("only shape-applicable candidates are probed"),
            owner_arguments: &candidate.owner_arguments,
            explicit_arguments: explicit_type_args,
            receiver_type: receiver.map(|receiver| receiver.ty),
            argument_types: &argument_types,
            expected_result: use_expected_result.then_some(expected_result).flatten(),
        }) {
            Ok(arguments) => arguments,
            Err(failure) => {
                return Err(Box::new(CandidateProbeFailure {
                    candidate: candidate_index,
                    state: Box::new(state),
                    arguments: lowered,
                    kind: CandidateProbeFailureKind::Constraint(failure),
                }));
            }
        };

        let mut args = Vec::with_capacity(lowered.len());
        for (index, argument) in lowered.into_iter().enumerate() {
            let argument = argument.expect("a successful candidate types every argument");
            let expected = state.substitute_call_level(candidate.params[index], &type_args);
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
                        source_index: index.saturating_sub(receiver_offset),
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

fn argument_types(arguments: &[Option<hir::Expr>], receiver_offset: usize) -> Vec<Option<TypeId>> {
    arguments[receiver_offset..]
        .iter()
        .map(|argument| argument.as_ref().map(|argument| argument.ty))
        .collect()
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
