//! Bind the actual supplied inputs without filling omitted parameters.

use super::*;
use crate::call_resolution::candidates::ArgumentMode;

impl<D: Copy> SuppliedArgumentMap<D> {
    pub(crate) fn bind(
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

        Ok(Self {
            receiver,
            parameters: mapped,
            source_order: (0..arguments.len())
                .map(SourceInputId::from_index)
                .collect(),
            positional_required_call,
        })
    }
}
