//! One typed argument probe for callable and nominal declarations.

use scoop_hir as hir;

use super::applicability::{DeclarationApplicabilityInput, DeclarationTypeArguments};
use super::arguments::CandidateArgumentMap;
use super::candidates::DeclarationSignature;
use super::contextual::{
    ArgumentExpression, ArgumentExpressionFailure, ArgumentInferenceFailure,
    ArgumentInferenceFailureKind, ArgumentInferenceInput, InferredArguments,
};
use super::solver::ConcreteInferenceArguments;
use crate::Lowerer;
use crate::expr::ResolvedCallTypeArgument;

pub(crate) struct CallInferenceInput<'a, D> {
    pub(crate) signature: &'a DeclarationSignature<D>,
    pub(crate) argument_map: &'a CandidateArgumentMap<D>,
    pub(crate) type_arguments: DeclarationTypeArguments<'a>,
    pub(crate) explicit_arguments: &'a [ResolvedCallTypeArgument],
    pub(crate) bound_receiver: Option<(hir::TypeId, hir::TypeId)>,
    pub(crate) expressions: &'a [ArgumentExpression<'a>],
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
    pub(crate) fn infer_call_arguments<D>(
        &mut self,
        input: CallInferenceInput<'_, D>,
    ) -> Result<InferredCall, Box<ArgumentInferenceFailure>> {
        let parameters = input
            .signature
            .owner_parameters
            .iter()
            .chain(&input.signature.callable_parameters)
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let patterns = input
            .argument_map
            .inference_patterns(&input.signature.value_parameters);
        let (mut session, environment) =
            self.declaration_applicability_session(DeclarationApplicabilityInput {
                owner_parameters: &input.signature.owner_parameters,
                callable_parameters: &input.signature.callable_parameters,
                type_arguments: input.type_arguments,
                explicit_arguments: input.explicit_arguments,
                bound_receiver: input.bound_receiver,
            });
        let InferredArguments {
            types,
            values,
            sinks,
        } = self.infer_contextual_arguments(ArgumentInferenceInput {
            expressions: input.expressions,
            patterns: &patterns,
            parameters: &parameters,
            session: &mut session,
            environment,
            expected_result: input
                .expected_result
                .map(|expected| (input.signature.return_type, expected)),
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
        for (index, (value, pattern)) in values.into_iter().zip(&patterns).enumerate() {
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
            .signature
            .value_parameters
            .iter()
            .map(|parameter| self.instantiate_method_ty(parameter.ty, &bindings))
            .collect();
        let return_type = self.instantiate_method_ty(input.signature.return_type, &bindings);
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
