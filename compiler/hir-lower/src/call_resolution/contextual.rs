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

impl CandidateArgumentMap {
    pub(crate) fn inference_patterns(&self, parameters: &[ValueParameter]) -> Vec<ArgumentPattern> {
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
    pub(crate) expected_result: Option<(hir::TypeId, hir::TypeId)>,
    pub(crate) forced_hint: Option<(usize, hir::TypeId)>,
}

pub(crate) struct InferredArguments {
    pub(crate) types: ConcreteInferenceArguments,
    pub(crate) values: Vec<hir::Expr>,
    pub(crate) sinks: Vec<Vec<hir::Statement>>,
}

pub(crate) struct ArgumentInferenceFailure {
    pub(crate) arguments: Vec<Option<hir::Expr>>,
    pub(crate) kind: ArgumentInferenceFailureKind,
}

pub(crate) enum ArgumentInferenceFailureKind {
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
            mut expected_result,
            forced_hint,
        } = input;
        debug_assert_eq!(expressions.len(), patterns.len());
        let mut values = vec![None; expressions.len()];
        let mut sinks = (0..expressions.len())
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        for (index, expression) in expressions.iter().enumerate() {
            if forced_hint.is_some_and(|(source, _)| source == index)
                || expression.requires_context(self)
            {
                continue;
            }
            match expression.lower(self, index, None) {
                Ok((value, sink)) => {
                    patterns[index].constrain(session, index, value.ty);
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
            let partial = self
                .solve_constraints_partially(session, environment)
                .map_err(|failure| {
                    Box::new(ArgumentInferenceFailure {
                        arguments: values.clone(),
                        kind: ArgumentInferenceFailureKind::Constraint(failure),
                    })
                })?;
            let complete = partial
                .owner
                .iter()
                .chain(&partial.callable)
                .all(Option::is_some);
            let hints = hint_bindings(parameters, partial);
            let mut progress = false;
            for (index, expression) in expressions.iter().enumerate() {
                if values[index].is_some() {
                    continue;
                }
                let hint = forced_hint
                    .filter(|(source, _)| *source == index)
                    .map(|(_, hint)| hint)
                    .or_else(|| self.try_substitute(patterns[index].ty, &hints));
                let Some(hint) = hint else { continue };
                match expression.lower(self, index, Some(hint)) {
                    Ok((value, sink)) => {
                        patterns[index].constrain(session, index, value.ty);
                        values[index] = Some(value);
                        sinks[index] = sink;
                        progress = true;
                    }
                    Err(failure) => {
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
            if let Some((result, expected)) = expected_result.take() {
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
            let mut first_failure = None;
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
        Ok(InferredArguments {
            types,
            values: values
                .into_iter()
                .map(|value| value.expect("a successful inference types every source input"))
                .collect(),
            sinks,
        })
    }
}

fn hint_bindings(
    parameters: &[hir::TypeParamId],
    partial: PartialInferenceArguments,
) -> Vec<Option<hir::TypeId>> {
    let length = parameters
        .iter()
        .map(|parameter| parameter.into_raw() as usize + 1)
        .max()
        .unwrap_or(0);
    let mut bindings = vec![None; length];
    for (parameter, value) in parameters
        .iter()
        .zip(partial.owner.into_iter().chain(partial.callable))
    {
        bindings[parameter.into_raw() as usize] = value;
    }
    bindings
}
