//! Shared fixed-point typing of source-ordered arguments for one candidate.

use scoop_hir as hir;

use super::arguments::{CandidateArgumentMap, SourceInputId, SourceInputKind, SuppliedArgumentMap};
use super::candidates::ValueParameter;
use super::constraints::{
    Constraint, ConstraintFailure, ConstraintOrigin, InferenceEnvironmentId, InferenceSession,
    TypeTerm,
};
use super::solver::{ConcreteInferenceArguments, PartialInferenceArguments};
use crate::Lowerer;
use crate::declaration_inference::DeclarationResult;

mod expressions;
mod fixed_point;
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

impl<D> SuppliedArgumentMap<D> {
    pub(crate) fn inference_patterns(
        &self,
        parameters: &[ValueParameter<D>],
    ) -> Vec<ArgumentPattern> {
        self.source_order
            .iter()
            .map(|source| {
                let (parameter, kind) = self.source_binding(*source);
                ArgumentPattern {
                    ty: parameters[parameter.index()].input_type(kind),
                    exact: kind == SourceInputKind::VarargArray,
                }
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

/// Candidate-local observations. The session still needs complete solving
/// before these facts can become an applicable call.
pub(crate) struct ContextualArgumentFacts {
    pub(crate) result: DeclarationResult,
    pub(crate) values: Vec<Option<hir::Expr>>,
    pub(crate) sinks: Vec<Vec<hir::Statement>>,
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
        let ContextualArgumentFacts {
            result,
            values,
            sinks,
        } = self.infer_contextual_argument_facts(ArgumentInferenceInput {
            session: &mut *input.session,
            ..input
        })?;
        let types = self
            .solve_constraints(input.session)
            .map_err(|failure| {
                Box::new(ArgumentInferenceFailure {
                    arguments: values.clone(),
                    kind: ArgumentInferenceFailureKind::Constraint(failure),
                })
            })?
            .arguments_for(input.session, input.environment);
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
