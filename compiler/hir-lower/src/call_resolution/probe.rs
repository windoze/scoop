//! One typed argument probe for callable and nominal declarations.

use scoop_hir as hir;

use super::applicability::DeclarationApplicabilityInput;
use super::contextual::{
    ArgumentExpression, ArgumentExpressionFailure, ArgumentInferenceFailure,
    ArgumentInferenceFailureKind, ArgumentInferenceInput, ArgumentPattern, InferredArguments,
};
use super::solver::ConcreteInferenceArguments;
use crate::Lowerer;

pub(crate) struct CallInferenceInput<'a> {
    pub(crate) declaration: DeclarationApplicabilityInput<'a>,
    pub(crate) parameter_types: &'a [hir::TypeId],
    pub(crate) return_type: hir::TypeId,
    pub(crate) expressions: &'a [ArgumentExpression<'a>],
    pub(crate) patterns: &'a [ArgumentPattern],
    pub(crate) expected_result: Option<hir::TypeId>,
    pub(crate) forced_hint: Option<(usize, hir::TypeId)>,
}

pub(crate) struct InferredCall {
    pub(crate) types: ConcreteInferenceArguments,
    pub(crate) bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    pub(crate) values: Vec<hir::Expr>,
    pub(crate) sinks: Vec<Vec<hir::Statement>>,
    pub(crate) parameter_types: Vec<hir::TypeId>,
    pub(crate) return_type: hir::TypeId,
    pub(crate) integer_arguments: Vec<Option<hir::IntegerKind>>,
}

impl Lowerer {
    pub(crate) fn infer_call_arguments(
        &mut self,
        input: CallInferenceInput<'_>,
    ) -> Result<InferredCall, Box<ArgumentInferenceFailure>> {
        let parameters = input
            .declaration
            .owner_parameters
            .iter()
            .chain(input.declaration.callable_parameters)
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let (mut session, environment) = self.declaration_applicability_session(input.declaration);
        let InferredArguments {
            types,
            values,
            sinks,
        } = self.infer_contextual_arguments(ArgumentInferenceInput {
            expressions: input.expressions,
            patterns: input.patterns,
            parameters: &parameters,
            session: &mut session,
            environment,
            expected_result: input
                .expected_result
                .map(|expected| (input.return_type, expected)),
            forced_hint: input.forced_hint,
        })?;
        let bindings = parameters
            .into_iter()
            .zip(types.owner.iter().chain(&types.callable).copied())
            .collect::<Vec<_>>();
        let integer_arguments = values
            .iter()
            .map(|value| match self.types[value.ty] {
                hir::Type::Integer(kind) => Some(kind),
                _ => None,
            })
            .collect();
        let mut adapted = Vec::with_capacity(values.len());
        for (index, (value, pattern)) in values.into_iter().zip(input.patterns).enumerate() {
            let forced = input.forced_hint.filter(|(source, _)| *source == index);
            let expected = forced.map_or_else(
                || self.instantiate_method_ty(pattern.ty, &bindings),
                |(_, hint)| hint,
            );
            // The solver checked declaration patterns. An intrinsic hint can
            // impose an additional concrete signature on that source input.
            if forced.is_some() && !self.is_subtype(value.ty, expected) {
                return Err(Box::new(ArgumentInferenceFailure {
                    arguments: adapted.into_iter().map(Some).collect(),
                    kind: ArgumentInferenceFailureKind::Expression(ArgumentExpressionFailure {
                        source_index: index,
                        expected: Some(expected),
                        context_dependent: true,
                        span: value.span,
                        reason: format!(
                            "expression has type {}, expected {}",
                            self.type_name(value.ty),
                            self.type_name(expected),
                        ),
                    }),
                }));
            }
            adapted.push(self.adapt_to(value, expected));
        }
        let parameter_types = input
            .parameter_types
            .iter()
            .map(|&ty| self.instantiate_method_ty(ty, &bindings))
            .collect();
        let return_type = self.instantiate_method_ty(input.return_type, &bindings);
        Ok(InferredCall {
            types,
            bindings,
            values: adapted,
            sinks,
            parameter_types,
            return_type,
            integer_arguments,
        })
    }
}
