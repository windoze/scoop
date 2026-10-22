//! Source-to-parameter mapping over declaration and call shapes.

use scoop_ast as ast;

use super::*;
use crate::call_resolution::candidates::ArgumentMode;

#[derive(Clone, Copy)]
pub(crate) struct ArgumentShape<'a> {
    pub(crate) name: Option<&'a str>,
    pub(crate) spread: bool,
    pub(crate) trailing: bool,
}

impl<'a> From<&'a ast::CallArgument> for ArgumentShape<'a> {
    fn from(argument: &'a ast::CallArgument) -> Self {
        Self {
            name: match &argument.name {
                ast::CallArgumentName::Positional | ast::CallArgumentName::TrailingLambda => None,
                ast::CallArgumentName::Named(name) => Some(&name.text),
            },
            spread: matches!(argument.spread, ast::SpreadSyntax::Spread(_)),
            trailing: matches!(argument.name, ast::CallArgumentName::TrailingLambda),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ParameterShape<'a, D> {
    pub(crate) name: &'a str,
    pub(crate) calling: ParameterCalling<D>,
}

#[derive(Clone, Copy)]
pub(crate) enum ParameterCalling<D> {
    Required,
    Default(D),
    Vararg { default: Option<D> },
}

impl<D: Copy> CandidateArgumentMap<D> {
    /// Operator-set syntax reserves its final input for the declared value
    /// parameter while mapping the preceding inputs with ordinary rules.
    pub(crate) fn operator_set<'a>(
        parameters: &[ParameterShape<'a, D>],
        arguments: &[ArgumentShape<'a>],
        receiver: ReceiverInput,
    ) -> Result<Self, ArgumentShapeFailure> {
        let Some(parameter) = parameters.last() else {
            return Err(ArgumentShapeFailure::Arity {
                expected: 1,
                supplied: arguments.len(),
            });
        };
        let mut arguments = arguments.to_vec();
        let Some(value) = arguments.last_mut() else {
            return Err(ArgumentShapeFailure::MissingRequired {
                name: parameter.name.to_owned(),
            });
        };
        value.name = Some(parameter.name);
        Self::map(parameters, ArgumentMode::Mixed, &arguments, receiver)
    }

    pub(crate) fn map(
        parameters: &[ParameterShape<'_, D>],
        mode: ArgumentMode,
        arguments: &[ArgumentShape<'_>],
        receiver: ReceiverInput,
    ) -> Result<Self, ArgumentShapeFailure> {
        SuppliedArgumentMap::bind(parameters, mode, arguments, receiver)?.finish(parameters)
    }
}

impl<D: Copy> SuppliedArgumentMap<D> {
    fn finish(
        self,
        parameters: &[ParameterShape<'_, D>],
    ) -> Result<CandidateArgumentMap<D>, ArgumentShapeFailure> {
        if self.positional_required_call && self.source_order.len() != parameters.len() {
            return Err(ArgumentShapeFailure::Arity {
                expected: parameters.len(),
                supplied: self.source_order.len(),
            });
        }

        let mut resolved = Vec::with_capacity(parameters.len());
        for (index, (parameter, input)) in parameters.iter().zip(self.parameters).enumerate() {
            let input = match input {
                Some(input) => input,
                None => match &parameter.calling {
                    ParameterCalling::Required => {
                        return Err(ArgumentShapeFailure::MissingRequired {
                            name: parameter.name.to_owned(),
                        });
                    }
                    ParameterCalling::Default(template) => {
                        ResolvedParameterInput::Default(*template)
                    }
                    ParameterCalling::Vararg { default } => {
                        ResolvedParameterInput::Vararg(match default {
                            None => ResolvedVarargInput::Empty,
                            Some(template) => ResolvedVarargInput::Default(*template),
                        })
                    }
                },
            };
            resolved.push(ParameterInput {
                parameter: ValueParameterId::from_index(index),
                input,
            });
        }
        Ok(CandidateArgumentMap {
            receiver: self.receiver,
            parameters: resolved,
            source_order: self.source_order,
        })
    }
}
