//! Shared fixed-point typing of source-ordered arguments for one candidate.

use scoop_hir as hir;

use super::arguments::{CandidateArgumentMap, SourceInputId, SourceInputKind};
use super::candidates::ValueParameter;
use super::constraints::{
    Constraint, ConstraintFailure, ConstraintOrigin, InferenceEnvironmentId, InferenceSession,
    TypeTerm,
};
use super::solver::{ConcreteInferenceArguments, PartialInferenceArguments};
use crate::Lowerer;
use crate::declaration_inference::DeclarationResult;

mod expressions;
pub(crate) use expressions::{ArgumentExpression, ArgumentExpressionFailure};

#[derive(Clone, Copy)]
pub(crate) struct ArgumentPattern {
    pub(crate) ty: hir::TypeId,
    pub(crate) exact: bool,
}

impl ArgumentPattern {
    pub(super) fn constrain(
        self,
        session: &mut InferenceSession,
        index: usize,
        actual: hir::TypeId,
    ) {
        let actual = TypeTerm::Rigid(actual);
        let expected = TypeTerm::Type(self.ty);
        session.push(
            if self.exact {
                Constraint::Equal(actual, expected)
            } else {
                Constraint::Subtype(actual, expected)
            },
            ConstraintOrigin::Argument(SourceInputId::from_index(index)),
        );
    }
}

impl<D> CandidateArgumentMap<D> {
    pub(crate) fn inference_patterns(
        &self,
        parameters: &[ValueParameter<D>],
    ) -> Vec<ArgumentPattern> {
        self.forwarding_parameter_types(parameters)
            .into_iter()
            .zip(&self.source_order)
            .map(|(ty, input)| ArgumentPattern {
                ty,
                exact: self.source_binding(*input).1 == SourceInputKind::VarargArray,
            })
            .collect()
    }
}

pub(crate) struct ArgumentInferenceInput<'a> {
    pub(crate) expressions: &'a [ArgumentExpression<'a>],
    pub(crate) patterns: &'a [ArgumentPattern],
    pub(crate) parameters: &'a [hir::TypeParamId],
    pub(crate) session: &'a mut InferenceSession,
    pub(crate) environment: InferenceEnvironmentId,
    pub(crate) result: DeclarationResult,
    pub(crate) expected_result: Option<hir::TypeId>,
    pub(crate) forced_hint: Option<(usize, hir::TypeId)>,
}

pub(crate) struct InferredArguments {
    pub(crate) result: DeclarationResult,
    pub(crate) types: ConcreteInferenceArguments,
    pub(crate) values: Vec<hir::Expr>,
    pub(crate) sinks: Vec<Vec<hir::Statement>>,
}

pub(crate) struct ArgumentInferenceFailure {
    pub(crate) arguments: Vec<Option<hir::Expr>>,
    pub(crate) kind: ArgumentInferenceFailureKind,
}

pub(crate) enum ArgumentInferenceFailureKind {
    Declaration {
        span: scoop_ast::Span,
        reason: String,
    },
    Constraint(ConstraintFailure),
    Expression(ArgumentExpressionFailure),
}

impl Lowerer {
    pub(crate) fn infer_contextual_arguments(
        &mut self,
        input: ArgumentInferenceInput<'_>,
    ) -> Result<InferredArguments, Box<ArgumentInferenceFailure>> {
        let ArgumentInferenceInput {
            expressions,
            patterns,
            parameters,
            session,
            environment,
            result,
            mut expected_result,
            forced_hint,
        } = input;
        debug_assert_eq!(expressions.len(), patterns.len());
        let mut resolved_result = None;
        let mut values = vec![None; expressions.len()];
        let mut sinks = (0..expressions.len())
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        let initial = self
            .solve_constraints_partially(session, environment)
            .map_err(|failure| {
                Box::new(ArgumentInferenceFailure {
                    arguments: values.clone(),
                    kind: ArgumentInferenceFailureKind::Constraint(failure),
                })
            })?;
        let initial_hints = hint_bindings(parameters, &initial);
        let mut previous = Some(initial);
        for (index, expression) in expressions.iter().enumerate() {
            if forced_hint.is_some_and(|(source, _)| source == index)
                || expression.requires_context(self)
            {
                continue;
            }
            let expected = expression.expectation(self, patterns[index].ty, &initial_hints);
            if matches!(
                expected,
                Some(crate::expr::ExpressionExpectation::Lambda(_))
            ) {
                continue;
            }
            match expression.lower(self, index, expected, false) {
                Ok((value, sink)) => {
                    patterns[index].constrain(session, index, value.ty);
                    previous = None;
                    values[index] = Some(value);
                    sinks[index] = sink;
                }
                Err(failure) => {
                    return Err(Box::new(ArgumentInferenceFailure {
                        arguments: values,
                        kind: ArgumentInferenceFailureKind::Expression(failure),
                    }));
                }
            }
        }

        loop {
            let partial = match previous.take() {
                Some(partial) => partial,
                None => self
                    .solve_constraints_partially(session, environment)
                    .map_err(|failure| {
                        Box::new(ArgumentInferenceFailure {
                            arguments: values.clone(),
                            kind: ArgumentInferenceFailureKind::Constraint(failure),
                        })
                    })?,
            };
            let complete = partial
                .owner
                .iter()
                .chain(&partial.callable)
                .all(Option::is_some);
            let hints = hint_bindings(parameters, &partial);
            let mut progress = false;
            let mut first_failure = None;
            for (index, expression) in expressions.iter().enumerate() {
                if values[index].is_some() {
                    continue;
                }
                let hint = forced_hint
                    .filter(|(source, _)| *source == index)
                    .map(|(_, hint)| crate::expr::ExpressionExpectation::Type(hint))
                    .or_else(|| expression.expectation(self, patterns[index].ty, &hints));
                let Some(hint) = hint else { continue };
                let partial_lambda = matches!(hint, crate::expr::ExpressionExpectation::Lambda(_));
                if partial_lambda && expected_result.is_some() {
                    continue;
                }
                let saved = partial_lambda.then(|| Box::new(self.clone()));
                match expression.lower(self, index, Some(hint), true) {
                    Ok((value, sink)) => {
                        patterns[index].constrain(session, index, value.ty);
                        values[index] = Some(value);
                        sinks[index] = sink;
                        progress = true;
                    }
                    Err(failure) => {
                        if let Some(mut saved) = saved {
                            std::mem::swap(self, &mut saved);
                            first_failure.get_or_insert((saved, failure));
                            continue;
                        }
                        return Err(Box::new(ArgumentInferenceFailure {
                            arguments: values,
                            kind: ArgumentInferenceFailureKind::Expression(failure),
                        }));
                    }
                }
            }
            if progress {
                continue;
            }
            if complete && values.iter().all(Option::is_some) {
                break;
            }
            if let Some(expected) = expected_result.take() {
                let result = self.infer_argument_result(result, &values)?;
                resolved_result = Some(result);
                session.push(
                    Constraint::Subtype(TypeTerm::Type(result), TypeTerm::Rigid(expected)),
                    ConstraintOrigin::ExpectedResult,
                );
                continue;
            }

            let mut seeds = values
                .iter()
                .enumerate()
                .filter_map(|(index, value)| value.is_none().then_some(index))
                .collect::<Vec<_>>();
            seeds.sort_by_key(|&index| std::cmp::Reverse(expressions[index].seed_rank(self)));
            for index in seeds {
                let expressions::ArgumentAttempt { mut state, result } =
                    expressions[index].try_default(self, index);
                match result {
                    Ok((value, sink)) => {
                        std::mem::swap(self, &mut state);
                        patterns[index].constrain(session, index, value.ty);
                        values[index] = Some(value);
                        sinks[index] = sink;
                        progress = true;
                        break;
                    }
                    Err(failure) => {
                        first_failure.get_or_insert((state, failure));
                    }
                }
            }
            if progress {
                continue;
            }
            if let Some((mut state, failure)) = first_failure {
                std::mem::swap(self, &mut state);
                return Err(Box::new(ArgumentInferenceFailure {
                    arguments: values,
                    kind: ArgumentInferenceFailureKind::Expression(failure),
                }));
            }
            break;
        }

        let types = self
            .solve_constraints(session)
            .map_err(|failure| {
                Box::new(ArgumentInferenceFailure {
                    arguments: values.clone(),
                    kind: ArgumentInferenceFailureKind::Constraint(failure),
                })
            })?
            .arguments_for(session, environment);
        let result = resolved_result.map_or(result, DeclarationResult::Known);
        Ok(InferredArguments {
            result,
            types,
            values: values
                .into_iter()
                .map(|value| value.expect("a successful inference types every source input"))
                .collect(),
            sinks,
        })
    }
}

mod result;

fn hint_bindings(
    parameters: &[hir::TypeParamId],
    partial: &PartialInferenceArguments,
) -> Vec<Option<hir::TypeId>> {
    let length = parameters
        .iter()
        .map(|parameter| parameter.into_raw() as usize + 1)
        .max()
        .unwrap_or(0);
    let mut bindings = vec![None; length];
    for (parameter, value) in parameters
        .iter()
        .zip(partial.owner.iter().chain(&partial.callable))
    {
        bindings[parameter.into_raw() as usize] = *value;
    }
    bindings
}
