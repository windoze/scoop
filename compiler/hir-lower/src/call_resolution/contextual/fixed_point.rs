//! Fixed-point expression facts, before complete constraint validation.

use super::*;
use crate::expr::ExpressionContextRequirement;

impl Lowerer {
    pub(crate) fn infer_contextual_argument_facts(
        &mut self,
        input: ArgumentInferenceInput<'_>,
    ) -> Result<ContextualArgumentFacts, Box<ArgumentInferenceFailure>> {
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
        let mut independent_failures = (0..expressions.len()).map(|_| None).collect::<Vec<_>>();
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
            if forced_hint.is_some_and(|(source, _)| source == index) {
                continue;
            }
            let expected = expression.expectation(self, patterns[index].ty, &initial_hints);
            if matches!(
                expected,
                Some(crate::expr::ExpressionExpectation::Lambda(_))
            ) {
                continue;
            }
            if expected.is_none() {
                let requirement = expression.context_requirement(self);
                if requirement == ExpressionContextRequirement::Required {
                    continue;
                }
                if requirement == ExpressionContextRequirement::Unresolved {
                    let expressions::ArgumentAttempt { mut state, result } =
                        expression.try_synthesize(self, index);
                    match result {
                        Ok((value, sink)) => {
                            std::mem::swap(self, &mut state);
                            patterns[index].constrain(session, index, value.ty);
                            previous = None;
                            values[index] = Some(value);
                            sinks[index] = sink;
                        }
                        Err(failure) => {
                            independent_failures[index] = Some((state, failure));
                        }
                    }
                    continue;
                }
            }
            let contextual = expected.is_some()
                && expression.context_requirement(self) == ExpressionContextRequirement::Required;
            match expression.lower(self, index, expected, contextual) {
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
                        independent_failures[index] = None;
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
                    match independent_failures[index].take() {
                        Some((state, failure)) => expressions::ArgumentAttempt {
                            state,
                            result: Err(failure),
                        },
                        None => expressions[index].try_synthesize(self, index),
                    };
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

        Ok(ContextualArgumentFacts {
            result: resolved_result.map_or(result, DeclarationResult::Known),
            values,
            sinks,
        })
    }
}
