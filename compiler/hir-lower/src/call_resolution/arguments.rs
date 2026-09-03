//! Candidate-specific source-to-parameter mapping.

use scoop_ast as ast;

use super::candidates::{
    ArgumentMode, CallableView, NominalConstructorView, ReceiverShape, ValueParameter,
};
use crate::defaults::{DefaultExprTemplateRef, SourceParameterCalling, SourceVarargOmission};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourceInputId(u32);

impl SourceInputId {
    pub(crate) fn from_index(index: usize) -> Self {
        Self(u32::try_from(index).expect("source argument index exceeds u32"))
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    #[cfg(test)]
    pub(super) fn from_test_index(index: usize) -> Self {
        Self::from_index(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ValueParameterId(u32);

impl ValueParameterId {
    fn from_index(index: usize) -> Self {
        Self(u32::try_from(index).expect("value parameter index exceeds u32"))
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiverInput {
    Absent,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParameterInput {
    pub(crate) parameter: ValueParameterId,
    pub(crate) input: ResolvedParameterInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedParameterInput {
    Explicit(SourceInputId),
    Default(DefaultExprTemplateRef),
    Vararg(ResolvedVarargInput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedVarargInput {
    Parts(Vec<VarargPart>),
    WholeArray(SourceInputId),
    Empty,
    Default(DefaultExprTemplateRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VarargPart {
    pub(crate) input: SourceInputId,
    pub(crate) kind: VarargPartKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarargPartKind {
    Element,
    CopyArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceInputKind {
    Value,
    VarargElement,
    VarargArray,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateArgumentMap {
    pub(crate) receiver: ReceiverInput,
    pub(crate) parameters: Vec<ParameterInput>,
    pub(crate) source_order: Vec<SourceInputId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArgumentShapeFailure {
    Arity { expected: usize, supplied: usize },
    UnknownName { name: String },
    DuplicateParameter { name: String },
    PositionalAfterNamed,
    PositionalForNamedOnly,
    NamedForPositionalOnly,
    SpreadForRegular { name: String },
    MissingRequired { name: String },
    MixedVarargInputs { name: String },
}

impl ArgumentShapeFailure {
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Arity { expected, supplied } => {
                format!("expects {expected} argument(s), but {supplied} were supplied")
            }
            Self::UnknownName { name } => format!("has no parameter named `{name}`"),
            Self::DuplicateParameter { name } => {
                format!("parameter `{name}` is supplied more than once")
            }
            Self::PositionalAfterNamed => {
                "an unnamed argument cannot follow an out-of-order named argument".to_string()
            }
            Self::PositionalForNamedOnly => {
                "named-field variants require every argument to be named".to_string()
            }
            Self::NamedForPositionalOnly => {
                "positional variants do not accept named arguments".to_string()
            }
            Self::SpreadForRegular { name } => {
                format!("spread is only allowed for vararg parameter `{name}`")
            }
            Self::MissingRequired { name } => {
                format!("required parameter `{name}` has no argument")
            }
            Self::MixedVarargInputs { name } => format!(
                "vararg parameter `{name}` cannot mix a named whole-array argument with element inputs"
            ),
        }
    }
}

impl CandidateArgumentMap {
    pub(crate) fn explicit_default_count(&self) -> usize {
        self.parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.input,
                    ResolvedParameterInput::Default(_)
                        | ResolvedParameterInput::Vararg(ResolvedVarargInput::Default(_))
                )
            })
            .count()
    }

    pub(crate) fn source(
        view: &CallableView,
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            &view.value_parameters,
            ArgumentMode::Mixed,
            arguments,
            match view.receiver {
                ReceiverShape::None | ReceiverShape::Instance => ReceiverInput::Absent,
                ReceiverShape::Extension(_) => ReceiverInput::Present,
            },
        )
    }

    /// Operator-set syntax reserves the final source input for the final
    /// non-vararg value parameter. Prefix inputs retain the ordinary M17
    /// default/vararg mapping rules.
    pub(crate) fn source_operator_set(
        view: &CallableView,
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        let Some(value_parameter) = view.value_parameters.last() else {
            return Err(ArgumentShapeFailure::Arity {
                expected: 1,
                supplied: arguments.len(),
            });
        };
        let mut arguments = arguments.to_vec();
        let Some(value) = arguments.last_mut() else {
            return Err(ArgumentShapeFailure::MissingRequired {
                name: value_parameter.name.clone(),
            });
        };
        value.name = ast::CallArgumentName::Named(ast::Ident {
            text: value_parameter.name.clone(),
            span: value.span,
        });
        Self::map(
            &view.value_parameters,
            ArgumentMode::Mixed,
            &arguments,
            match view.receiver {
                ReceiverShape::None | ReceiverShape::Instance => ReceiverInput::Absent,
                ReceiverShape::Extension(_) => ReceiverInput::Present,
            },
        )
    }

    pub(crate) fn source_nominal(
        view: &NominalConstructorView,
        arguments: &[ast::CallArgument],
    ) -> Result<Self, ArgumentShapeFailure> {
        Self::map(
            &view.value_parameters,
            view.argument_mode,
            arguments,
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
        Ok(Self::positional(
            expected,
            match view.receiver {
                ReceiverShape::None | ReceiverShape::Instance => ReceiverInput::Absent,
                ReceiverShape::Extension(_) => ReceiverInput::Present,
            },
        ))
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

    fn map(
        parameters: &[ValueParameter],
        mode: ArgumentMode,
        arguments: &[ast::CallArgument],
        receiver: ReceiverInput,
    ) -> Result<Self, ArgumentShapeFailure> {
        let positional_required_call = arguments
            .iter()
            .all(|argument| matches!(argument.name, ast::CallArgumentName::Positional))
            && parameters
                .iter()
                .all(|parameter| matches!(parameter.calling, SourceParameterCalling::Required));
        let mut mapped: Vec<Option<ResolvedParameterInput>> = vec![None; parameters.len()];
        let mut next = 0;
        let mut named_only = mode == ArgumentMode::NamedOnly;

        for (source_index, argument) in arguments.iter().enumerate() {
            let source = SourceInputId::from_index(source_index);
            match &argument.name {
                ast::CallArgumentName::Positional => {
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
                        SourceParameterCalling::Required | SourceParameterCalling::Default(_) => {
                            if matches!(argument.spread, ast::SpreadSyntax::Spread(_)) {
                                return Err(ArgumentShapeFailure::SpreadForRegular {
                                    name: parameter.name.clone(),
                                });
                            }
                            mapped[next] = Some(ResolvedParameterInput::Explicit(source));
                            next += 1;
                        }
                        SourceParameterCalling::Vararg { .. } => {
                            let part = VarargPart {
                                input: source,
                                kind: match argument.spread {
                                    ast::SpreadSyntax::Plain => VarargPartKind::Element,
                                    ast::SpreadSyntax::Spread(_) => VarargPartKind::CopyArray,
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
                                        name: parameter.name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                ast::CallArgumentName::Named(name) => {
                    if mode == ArgumentMode::PositionalOnly {
                        return Err(ArgumentShapeFailure::NamedForPositionalOnly);
                    }
                    let Some(index) = parameters
                        .iter()
                        .position(|parameter| parameter.name == name.text)
                    else {
                        return Err(ArgumentShapeFailure::UnknownName {
                            name: name.text.clone(),
                        });
                    };
                    let parameter = &parameters[index];
                    if mapped[index].is_some() {
                        return Err(ArgumentShapeFailure::DuplicateParameter {
                            name: parameter.name.clone(),
                        });
                    }
                    let input = match parameter.calling {
                        SourceParameterCalling::Required | SourceParameterCalling::Default(_) => {
                            if matches!(argument.spread, ast::SpreadSyntax::Spread(_)) {
                                return Err(ArgumentShapeFailure::SpreadForRegular {
                                    name: parameter.name.clone(),
                                });
                            }
                            ResolvedParameterInput::Explicit(source)
                        }
                        SourceParameterCalling::Vararg { .. } => {
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
                    if matches!(parameter.calling, SourceParameterCalling::Vararg { .. }) {
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
                    SourceParameterCalling::Required => {
                        return Err(ArgumentShapeFailure::MissingRequired {
                            name: parameter.name.clone(),
                        });
                    }
                    SourceParameterCalling::Default(template) => {
                        ResolvedParameterInput::Default(*template)
                    }
                    SourceParameterCalling::Vararg { omission, .. } => {
                        ResolvedParameterInput::Vararg(match omission {
                            SourceVarargOmission::EmptyArray => ResolvedVarargInput::Empty,
                            SourceVarargOmission::Default(template) => {
                                ResolvedVarargInput::Default(*template)
                            }
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

    pub(crate) fn source_binding(
        &self,
        source: SourceInputId,
    ) -> (ValueParameterId, SourceInputKind) {
        for parameter in &self.parameters {
            match &parameter.input {
                ResolvedParameterInput::Explicit(input) if *input == source => {
                    return (parameter.parameter, SourceInputKind::Value);
                }
                ResolvedParameterInput::Vararg(ResolvedVarargInput::WholeArray(input))
                    if *input == source =>
                {
                    return (parameter.parameter, SourceInputKind::VarargArray);
                }
                ResolvedParameterInput::Vararg(ResolvedVarargInput::Parts(parts)) => {
                    if let Some(part) = parts.iter().find(|part| part.input == source) {
                        let kind = match part.kind {
                            VarargPartKind::Element => SourceInputKind::VarargElement,
                            VarargPartKind::CopyArray => SourceInputKind::VarargArray,
                        };
                        return (parameter.parameter, kind);
                    }
                }
                ResolvedParameterInput::Explicit(_)
                | ResolvedParameterInput::Default(_)
                | ResolvedParameterInput::Vararg(
                    ResolvedVarargInput::WholeArray(_)
                    | ResolvedVarargInput::Empty
                    | ResolvedVarargInput::Default(_),
                ) => {}
            }
        }
        unreachable!("each source argument is bound exactly once")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_resolution::candidates::{
        CallableEffects, CallableSource, NominalConstructorSource, SourceDispatch, ValueParameter,
    };
    use crate::defaults::{DefaultExprTemplateRef, SourceParameterCalling, SourceVarargOmission};
    use scoop_ast::Span;
    use scoop_hir as hir;

    fn argument(name: Option<&str>, spread: bool) -> ast::CallArgument {
        let span = Span::new(0, 0);
        ast::CallArgument {
            name: name.map_or(ast::CallArgumentName::Positional, |name| {
                ast::CallArgumentName::Named(ast::Ident {
                    text: name.to_string(),
                    span,
                })
            }),
            spread: if spread {
                ast::SpreadSyntax::Spread(span)
            } else {
                ast::SpreadSyntax::Plain
            },
            expression: ast::Expr::UnitLiteral { span },
            span,
        }
    }

    fn view(parameter_count: usize, receiver: ReceiverShape) -> CallableView {
        CallableView {
            target: CallableSource::Free(hir::FunctionId::from_raw(0_u32.into())),
            receiver,
            owner_parameters: Vec::new(),
            callable_parameters: Vec::new(),
            value_parameters: (0..parameter_count)
                .map(|index| ValueParameter {
                    name: format!("p{index}"),
                    calling: SourceParameterCalling::Required,
                    ty: hir::TypeId::from_raw((index as u32).into()),
                })
                .collect(),
            return_type: hir::TypeId::from_raw(0_u32.into()),
            effects: CallableEffects {
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
            },
            dispatch: SourceDispatch::Direct,
            declaration_span: Span::new(0, 0),
        }
    }

    fn template(index: u32) -> DefaultExprTemplateRef {
        DefaultExprTemplateRef::Export(hir::ExportDefaultSourceId::from_raw(index.into()))
    }

    #[test]
    fn exact_mapping_is_candidate_owned_and_preserves_source_order() {
        let map = CandidateArgumentMap::exact_lowered(
            &view(
                2,
                ReceiverShape::Extension(hir::TypeId::from_raw(3_u32.into())),
            ),
            2,
        )
        .expect("matching positional arguments");
        assert_eq!(map.receiver, ReceiverInput::Present);
        assert_eq!(map.parameters[0].parameter.index(), 0);
        assert_eq!(
            map.parameters[1].input,
            ResolvedParameterInput::Explicit(SourceInputId::from_test_index(1))
        );
        assert_eq!(
            map.source_order
                .iter()
                .map(|input| input.index())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn exact_mapping_reports_candidate_arity() {
        assert_eq!(
            CandidateArgumentMap::exact_lowered(&view(2, ReceiverShape::None), 1),
            Err(ArgumentShapeFailure::Arity {
                expected: 2,
                supplied: 1,
            })
        );
    }

    #[test]
    fn exact_nominal_mapping_uses_constructor_fields() {
        let view = NominalConstructorView {
            target: NominalConstructorSource::Struct(hir::StructConstructorId::from_raw(
                0_u32.into(),
            )),
            owner_parameters: Vec::new(),
            value_parameters: vec![
                ValueParameter {
                    name: "left".to_string(),
                    calling: SourceParameterCalling::Required,
                    ty: hir::TypeId::from_raw(0_u32.into()),
                },
                ValueParameter {
                    name: "right".to_string(),
                    calling: SourceParameterCalling::Required,
                    ty: hir::TypeId::from_raw(0_u32.into()),
                },
            ],
            argument_mode: ArgumentMode::Mixed,
            result_type: hir::TypeId::from_raw(0_u32.into()),
            declaration_span: Span::new(0, 0),
        };

        let args = [argument(None, false), argument(None, false)];
        let mapping = CandidateArgumentMap::source_nominal(&view, &args).expect("matching arity");
        assert_eq!(
            mapping.parameters[0].input,
            ResolvedParameterInput::Explicit(SourceInputId::from_test_index(0))
        );
        assert_eq!(mapping.parameters[1].parameter.index(), 1);
        assert_eq!(
            CandidateArgumentMap::source_nominal(&view, &[argument(None, false)]),
            Err(ArgumentShapeFailure::Arity {
                expected: 2,
                supplied: 1,
            })
        );
    }

    #[test]
    fn source_mapping_supports_ordered_named_defaults_and_varargs() {
        let ty = hir::TypeId::from_raw(0_u32.into());
        let element = hir::TypeId::from_raw(1_u32.into());
        let mut view = view(0, ReceiverShape::None);
        view.value_parameters = vec![
            ValueParameter {
                name: "first".to_string(),
                calling: SourceParameterCalling::Required,
                ty,
            },
            ValueParameter {
                name: "middle".to_string(),
                calling: SourceParameterCalling::Default(template(0)),
                ty,
            },
            ValueParameter {
                name: "values".to_string(),
                calling: SourceParameterCalling::Vararg {
                    element_type: element,
                    array_type: ty,
                    omission: SourceVarargOmission::EmptyArray,
                },
                ty,
            },
            ValueParameter {
                name: "last".to_string(),
                calling: SourceParameterCalling::Default(template(1)),
                ty,
            },
        ];

        let args = [
            argument(Some("first"), false),
            argument(Some("middle"), false),
            argument(None, false),
            argument(None, true),
            argument(Some("last"), false),
        ];
        let mapping = CandidateArgumentMap::source(&view, &args).expect("candidate shape");
        assert!(matches!(
            &mapping.parameters[2].input,
            ResolvedParameterInput::Vararg(ResolvedVarargInput::Parts(parts))
                if parts.len() == 2
                    && parts[0].kind == VarargPartKind::Element
                    && parts[1].kind == VarargPartKind::CopyArray
        ));
    }

    #[test]
    fn jumping_named_argument_makes_the_remaining_tail_named_only() {
        let view = view(3, ReceiverShape::None);
        let failure = CandidateArgumentMap::source(
            &view,
            &[argument(Some("p2"), false), argument(None, false)],
        )
        .expect_err("positional input after a jump is illegal");
        assert_eq!(failure, ArgumentShapeFailure::PositionalAfterNamed);
    }
}
