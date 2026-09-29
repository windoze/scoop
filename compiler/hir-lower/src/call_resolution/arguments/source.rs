//! Shape adapters for checked source declaration views.

use scoop_ast as ast;

use super::*;
use crate::call_resolution::candidates::{
    ArgumentMode, CallableView, NominalConstructorView, ReceiverShape,
};
use crate::defaults::SourceVarargOmission;

impl CandidateArgumentMap {
    pub(crate) fn source(
        view: &CallableView,
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            &parameter_shapes(&view.value_parameters),
            ArgumentMode::Mixed,
            &arguments
                .iter()
                .map(ArgumentShape::from)
                .collect::<Vec<_>>(),
            receiver_input(view.receiver),
        )
    }

    pub(crate) fn source_operator_set(
        view: &CallableView,
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::operator_set(
            &parameter_shapes(&view.value_parameters),
            &arguments
                .iter()
                .map(ArgumentShape::from)
                .collect::<Vec<_>>(),
            receiver_input(view.receiver),
        )
    }

    pub(crate) fn source_nominal(
        view: &NominalConstructorView,
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            &parameter_shapes(&view.value_parameters),
            view.argument_mode,
            &arguments
                .iter()
                .map(ArgumentShape::from)
                .collect::<Vec<_>>(),
            ReceiverInput::Absent,
        )
    }

    pub(crate) fn exact_lowered(
        view: &CallableView,
        supplied: usize,
    ) -> Result<Self, ArgumentShapeFailure> {
        let expected = view.value_parameters.len();
        if expected != supplied {
            return Err(ArgumentShapeFailure::Arity { expected, supplied });
        }
        Ok(Self::positional(expected, receiver_input(view.receiver)))
    }

    pub(crate) fn positional(parameter_count: usize, receiver: ReceiverInput) -> Self {
        let supplied = parameter_count;
        let source_order = (0..supplied).map(SourceInputId::from_index).collect();
        let parameters = (0..supplied)
            .map(|index| ParameterInput {
                parameter: ValueParameterId::from_index(index),
                input: ResolvedParameterInput::Explicit(SourceInputId::from_index(index)),
            })
            .collect();
        Self {
            receiver,
            parameters,
            source_order,
        }
    }
}

fn receiver_input(receiver: ReceiverShape) -> ReceiverInput {
    match receiver {
        ReceiverShape::None | ReceiverShape::Instance => ReceiverInput::Absent,
        ReceiverShape::Extension(_) => ReceiverInput::Present,
    }
}

fn parameter_shapes(
    parameters: &[ValueParameter],
) -> Vec<ParameterShape<'_, DefaultArgumentSource>> {
    parameters
        .iter()
        .map(|parameter| ParameterShape {
            name: &parameter.name,
            calling: match parameter.calling {
                SourceParameterCalling::Required => ParameterCalling::Required,
                SourceParameterCalling::Default(template) => ParameterCalling::Default(template),
                SourceParameterCalling::Vararg { omission, .. } => ParameterCalling::Vararg {
                    default: match omission {
                        SourceVarargOmission::EmptyArray => None,
                        SourceVarargOmission::Default(template) => Some(template),
                    },
                },
            },
        })
        .collect()
}
