//! Shape adapters for checked source declaration views.

use scoop_ast as ast;

use super::*;
use crate::call_resolution::candidates::VarargOmission;
use crate::call_resolution::candidates::{
    ArgumentMode, CallableView, NominalConstructorView, ReceiverShape,
};

impl CandidateArgumentMap {
    pub(crate) fn source(
        view: &CallableView,
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            &parameter_shapes(&view.signature.value_parameters),
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
            &parameter_shapes(&view.signature.value_parameters),
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
            &parameter_shapes(&view.signature.value_parameters),
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
        let expected = view.signature.value_parameters.len();
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

impl<D: Copy> CandidateArgumentMap<D> {
    pub(crate) fn declaration(
        parameters: &[ValueParameter<D>],
        mode: ArgumentMode,
        arguments: &[ArgumentShape<'_>],
        receiver: ReceiverInput,
        operator_set: bool,
    ) -> Result<Self, ArgumentShapeFailure> {
        let shapes = parameter_shapes(parameters);
        if operator_set {
            Self::operator_set(&shapes, arguments, receiver)
        } else {
            Self::map(&shapes, mode, arguments, receiver)
        }
    }
}

fn parameter_shapes<D: Copy>(parameters: &[ValueParameter<D>]) -> Vec<ParameterShape<'_, D>> {
    parameters
        .iter()
        .map(|parameter| ParameterShape {
            name: &parameter.name,
            calling: match parameter.calling {
                ValueParameterCalling::Required => ParameterCalling::Required,
                ValueParameterCalling::Default(template) => ParameterCalling::Default(template),
                ValueParameterCalling::Vararg { omission, .. } => ParameterCalling::Vararg {
                    default: match omission {
                        VarargOmission::EmptyArray => None,
                        VarargOmission::Default(template) => Some(template),
                    },
                },
            },
        })
        .collect()
}
