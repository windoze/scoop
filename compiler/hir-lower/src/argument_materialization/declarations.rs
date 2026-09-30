//! Declaration storage supplies resolved types and defaults to one evaluator.

use super::*;
use crate::call_resolution::arguments::ResolvedParameterInput;

impl Lowerer {
    pub(crate) fn materialize_callable_arguments(
        &mut self,
        request: CallableArgumentMaterialization<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<(Option<hir::Expr>, Vec<hir::Expr>)> {
        let signature = self.signatures[&request.function].clone();
        let bindings = signature
            .type_params
            .iter()
            .zip(request.type_args)
            .map(|(parameter, &argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        let parameters = signature
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let ty = if index == 0
                    && self
                        .foreign_callback_core
                        .is_some_and(|core| core.register == request.function)
                {
                    let ResolvedParameterInput::Explicit(source) =
                        request.argument_map.parameters[index].input
                    else {
                        unreachable!("foreign callback input is required")
                    };
                    request.source_args[source.index()].ty
                } else {
                    parameter.ty
                };
                (
                    parameter.name.text.clone(),
                    self.instantiate_method_ty(ty, &bindings),
                )
            })
            .collect::<Vec<_>>();
        self.materialize_argument_inputs(
            ResolvedArgumentMaterialization {
                parameters: &parameters,
                inputs: &request.argument_map.parameters,
                receiver: request.receiver,
                source_args: request.source_args,
                argument_sinks: request.argument_sinks,
                call_span: request.call_span,
                evaluation: request.evaluation,
                temporary_prefix: "$",
            },
            sink,
            |state, source, context| {
                state.instantiate_default(
                    *source,
                    &bindings,
                    context.receiver,
                    context.parameters,
                    context.call_span,
                    context.sink,
                )
            },
        )
    }

    pub(crate) fn materialize_nominal_arguments(
        &mut self,
        request: NominalArgumentMaterialization<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<Vec<hir::Expr>> {
        let bindings = request
            .view
            .owner_parameters
            .iter()
            .zip(request.type_args)
            .map(|(parameter, &argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        let parameters = request
            .view
            .value_parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.name.clone(),
                    self.instantiate_method_ty(parameter.ty, &bindings),
                )
            })
            .collect::<Vec<_>>();
        self.materialize_argument_inputs(
            ResolvedArgumentMaterialization {
                parameters: &parameters,
                inputs: &request.argument_map.parameters,
                receiver: None,
                source_args: request.source_args,
                argument_sinks: request.argument_sinks,
                call_span: request.call_span,
                evaluation: ArgumentEvaluation::Source,
                temporary_prefix: "$",
            },
            sink,
            |state, source, context| {
                state.instantiate_default(
                    *source,
                    &bindings,
                    context.receiver,
                    context.parameters,
                    context.call_span,
                    context.sink,
                )
            },
        )
        .map(|(_, arguments)| arguments)
    }
}
