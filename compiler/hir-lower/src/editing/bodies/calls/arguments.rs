use super::*;
use crate::call_resolution::arguments::{ParameterInput, ResolvedParameterInput, SourceInputId};

impl Lowerer {
    pub(crate) fn observe_materialized_arguments<D>(
        &mut self,
        span: ast::Span,
        parameters: &[(String, hir::TypeId)],
        inputs: &[ParameterInput<D>],
        source_count: usize,
    ) {
        let Some(source) = self.observed_call_input(span) else {
            return;
        };
        // A synthesized helper can share its containing source span while
        // consuming different inputs. Only the actual AST call is observed.
        if source.arguments.len() != source_count {
            return;
        }
        let arguments = source
            .arguments
            .iter()
            .enumerate()
            .map(|(index, &(span, trailing, name_span))| {
                let (parameter, _) = crate::call_resolution::arguments::source_binding(
                    inputs,
                    SourceInputId::from_index(index),
                );
                hir::EditingCallArgument {
                    span,
                    name_span,
                    parameter: parameter.index(),
                    trailing,
                }
            })
            .collect();
        let parameters = parameters
            .iter()
            .zip(inputs)
            .map(|((name, value_type), input)| hir::EditingAppliedParameter {
                name: Some(name.clone()),
                value_type: *value_type,
                vararg_element_type: matches!(input.input, ResolvedParameterInput::Vararg(_)).then(
                    || {
                        self.array_element_ty(*value_type)
                            .expect("a checked vararg parameter has its array type")
                    },
                ),
            })
            .collect();
        self.editing_body
            .as_mut()
            .expect("active query")
            .argument_maps
            .insert(
                span,
                CheckedArguments {
                    parameters,
                    arguments,
                },
            );
    }

    pub(crate) fn observe_function_call_arguments(
        &mut self,
        span: ast::Span,
        parameters: impl Iterator<Item = hir::TypeId>,
    ) {
        let Some(source) = self.observed_call_input(span) else {
            return;
        };
        let arguments = source
            .arguments
            .iter()
            .enumerate()
            .map(
                |(parameter, &(span, trailing, name_span))| hir::EditingCallArgument {
                    span,
                    name_span,
                    parameter,
                    trailing,
                },
            )
            .collect();
        let parameters = parameters
            .map(|value_type| hir::EditingAppliedParameter {
                name: None,
                value_type,
                vararg_element_type: None,
            })
            .collect();
        self.editing_body
            .as_mut()
            .expect("active query")
            .argument_maps
            .insert(
                span,
                CheckedArguments {
                    parameters,
                    arguments,
                },
            );
    }

    fn observed_call_input(&self, span: ast::Span) -> Option<CallInput> {
        if !self.observes_current_body() {
            return None;
        }
        let body = self.editing_body.as_ref().expect("active body query");
        if body.failed || self.diagnostics.len() != body.diagnostics_start {
            return None;
        }
        body.call_inputs.get(&span).cloned()
    }
}
