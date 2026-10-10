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
        let positional_required_call = arguments.iter().all(|argument| argument.name.is_none())
            && parameters
                .iter()
                .all(|parameter| matches!(parameter.calling, ParameterCalling::Required));
        let mut mapped: Vec<Option<ResolvedParameterInput<D>>> = vec![None; parameters.len()];
        if let Some((index, _)) = arguments
            .iter()
            .enumerate()
            .find(|(_, argument)| argument.trailing)
        {
            let Some(parameter) = parameters.last() else {
                return Err(ArgumentShapeFailure::Arity {
                    expected: 0,
                    supplied: arguments.len(),
                });
            };
            if matches!(parameter.calling, ParameterCalling::Vararg { .. }) {
                return Err(ArgumentShapeFailure::TrailingForVararg {
                    name: parameter.name.to_owned(),
                });
            }
            mapped[parameters.len() - 1] = Some(ResolvedParameterInput::Explicit(
                SourceInputId::from_index(index),
            ));
        }
        let mut next = 0;
        let mut named_only = mode == ArgumentMode::NamedOnly;

        for (source_index, argument) in arguments.iter().enumerate() {
            if argument.trailing {
                continue;
            }
            let source = SourceInputId::from_index(source_index);
            match argument.name {
                None => {
                    if mode == ArgumentMode::NamedOnly {
                        return Err(ArgumentShapeFailure::PositionalForNamedOnly);
                    }
                    if named_only {
                        return Err(ArgumentShapeFailure::PositionalAfterNamed);
                    }
                    let Some(parameter) = parameters.get(next) else {
                        return Err(ArgumentShapeFailure::Arity {
                            expected: parameters.len(),
                            supplied: arguments.len(),
                        });
                    };
                    match &parameter.calling {
                        ParameterCalling::Required | ParameterCalling::Default(_) => {
                            if mapped[next].is_some() {
                                return Err(ArgumentShapeFailure::DuplicateParameter {
                                    name: parameter.name.to_owned(),
                                });
                            }
                            if argument.spread {
                                return Err(ArgumentShapeFailure::SpreadForRegular {
                                    name: parameter.name.to_owned(),
                                });
                            }
                            mapped[next] = Some(ResolvedParameterInput::Explicit(source));
                            next += 1;
                        }
                        ParameterCalling::Vararg { .. } => {
                            let part = VarargPart {
                                input: source,
                                kind: match argument.spread {
                                    false => VarargPartKind::Element,
                                    true => VarargPartKind::CopyArray,
                                },
                            };
                            match &mut mapped[next] {
                                None => {
                                    mapped[next] = Some(ResolvedParameterInput::Vararg(
                                        ResolvedVarargInput::Parts(vec![part]),
                                    ));
                                }
                                Some(ResolvedParameterInput::Vararg(
                                    ResolvedVarargInput::Parts(parts),
                                )) => parts.push(part),
                                Some(_) => {
                                    return Err(ArgumentShapeFailure::MixedVarargInputs {
                                        name: parameter.name.to_owned(),
                                    });
                                }
                            }
                        }
                    }
                }
                Some(name) => {
                    if mode == ArgumentMode::PositionalOnly {
                        return Err(ArgumentShapeFailure::NamedForPositionalOnly);
                    }
                    let Some(index) = parameters
                        .iter()
                        .position(|parameter| parameter.name == name)
                    else {
                        return Err(ArgumentShapeFailure::UnknownName {
                            name: name.to_owned(),
                        });
                    };
                    let parameter = &parameters[index];
                    if mapped[index].is_some() {
                        return Err(ArgumentShapeFailure::DuplicateParameter {
                            name: parameter.name.to_owned(),
                        });
                    }
                    let input = match parameter.calling {
                        ParameterCalling::Required | ParameterCalling::Default(_) => {
                            if argument.spread {
                                return Err(ArgumentShapeFailure::SpreadForRegular {
                                    name: parameter.name.to_owned(),
                                });
                            }
                            ResolvedParameterInput::Explicit(source)
                        }
                        ParameterCalling::Vararg { .. } => {
                            ResolvedParameterInput::Vararg(ResolvedVarargInput::WholeArray(source))
                        }
                    };
                    mapped[index] = Some(input);
                    if index == next {
                        next += 1;
                        while next < mapped.len() && mapped[next].is_some() {
                            next += 1;
                        }
                    } else {
                        named_only = true;
                    }
                    if matches!(parameter.calling, ParameterCalling::Vararg { .. }) {
                        named_only = true;
                    }
                }
            }
        }

        if positional_required_call && arguments.len() != parameters.len() {
            return Err(ArgumentShapeFailure::Arity {
                expected: parameters.len(),
                supplied: arguments.len(),
            });
        }

        let mut resolved = Vec::with_capacity(parameters.len());
        for (index, (parameter, input)) in parameters.iter().zip(mapped).enumerate() {
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
        Ok(Self {
            receiver,
            parameters: resolved,
            source_order: (0..arguments.len())
                .map(SourceInputId::from_index)
                .collect(),
        })
    }
}
