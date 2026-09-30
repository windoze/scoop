//! Preserve source evaluation order before assembling declaration parameters.

use super::*;
use crate::call_resolution::arguments::{
    ResolvedParameterInput, ResolvedVarargInput, VarargPartKind,
};

impl Lowerer {
    pub(crate) fn materialize_argument_inputs<D>(
        &mut self,
        request: ResolvedArgumentMaterialization<'_, D>,
        sink: &mut Vec<hir::Statement>,
        mut default: impl FnMut(&mut Self, &D, DefaultArgumentEvaluation<'_>) -> Option<hir::Expr>,
    ) -> Option<(Option<hir::Expr>, Vec<hir::Expr>)> {
        let ResolvedArgumentMaterialization {
            parameters,
            inputs,
            receiver,
            source_args,
            mut argument_sinks,
            call_span,
            evaluation,
            temporary_prefix,
        } = request;
        let receiver = receiver.map(|value| {
            self.evaluate_call_argument(
                format!("{temporary_prefix}receiver"),
                value,
                call_span,
                sink,
                evaluation,
            )
        });
        let source_args = source_args
            .into_iter()
            .enumerate()
            .map(|(index, argument)| {
                sink.append(&mut argument_sinks[index]);
                self.evaluate_call_argument(
                    format!("{temporary_prefix}argument.{index}"),
                    argument,
                    call_span,
                    sink,
                    evaluation,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(inputs.len(), parameters.len());
        let mut values = Vec::with_capacity(parameters.len());
        for (input, (name, parameter)) in inputs.iter().zip(parameters) {
            let value = match &input.input {
                ResolvedParameterInput::Explicit(source)
                | ResolvedParameterInput::Vararg(ResolvedVarargInput::WholeArray(source)) => {
                    source_args[source.index()].clone()
                }
                ResolvedParameterInput::Default(source)
                | ResolvedParameterInput::Vararg(ResolvedVarargInput::Default(source)) => default(
                    self,
                    source,
                    DefaultArgumentEvaluation {
                        receiver: receiver.as_ref(),
                        parameters: &values,
                        call_span,
                        sink,
                    },
                )?,
                ResolvedParameterInput::Vararg(ResolvedVarargInput::Empty) => {
                    let element = self
                        .array_element_ty(*parameter)
                        .expect("a resolved vararg parameter has an array element type");
                    self.array_assembly(element, *parameter, Vec::new(), call_span)
                }
                ResolvedParameterInput::Vararg(ResolvedVarargInput::Parts(parts)) => {
                    let element = self
                        .array_element_ty(*parameter)
                        .expect("a resolved vararg parameter has an array element type");
                    let parts = parts
                        .iter()
                        .map(|part| {
                            let value = source_args[part.input.index()].clone();
                            match part.kind {
                                VarargPartKind::Element => {
                                    hir::ArrayAssemblyPart::Element(self.adapt_to(value, element))
                                }
                                VarargPartKind::CopyArray => {
                                    hir::ArrayAssemblyPart::CopyArray(value)
                                }
                            }
                        })
                        .collect();
                    self.array_assembly(element, *parameter, parts, call_span)
                }
            };
            let value = self.adapt_to(value, *parameter);
            values.push(self.evaluate_call_argument(
                format!("{temporary_prefix}parameter.{name}"),
                value,
                call_span,
                sink,
                evaluation,
            ));
        }
        Some((receiver, values))
    }

    fn evaluate_call_argument(
        &mut self,
        name: String,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        evaluation: ArgumentEvaluation,
    ) -> hir::Expr {
        match evaluation {
            ArgumentEvaluation::Source => self.materialize_temporary(name, value, span, sink),
            ArgumentEvaluation::Lowered => value,
        }
    }
}
