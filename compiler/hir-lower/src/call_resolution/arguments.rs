//! Candidate-specific source-to-parameter mapping.

use super::candidates::ValueParameter;
use crate::call_resolution::candidates::ValueParameterCalling;
use crate::defaults::DefaultArgumentSource;

mod mapping;
mod source;
#[cfg(test)]
mod tests;

pub(crate) use mapping::{ArgumentShape, ParameterCalling, ParameterShape};

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
pub(crate) struct ParameterInput<D = DefaultArgumentSource> {
    pub(crate) parameter: ValueParameterId,
    pub(crate) input: ResolvedParameterInput<D>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedParameterInput<D = DefaultArgumentSource> {
    Explicit(SourceInputId),
    Default(D),
    Vararg(ResolvedVarargInput<D>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedVarargInput<D = DefaultArgumentSource> {
    Parts(Vec<VarargPart>),
    WholeArray(SourceInputId),
    Empty,
    Default(D),
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
pub(crate) struct CandidateArgumentMap<D = DefaultArgumentSource> {
    pub(crate) receiver: ReceiverInput,
    pub(crate) parameters: Vec<ParameterInput<D>>,
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
    TrailingForVararg { name: String },
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
            Self::TrailingForVararg { name } => format!(
                "trailing lambda cannot bind vararg parameter `{name}`; pass it inside parentheses"
            ),
        }
    }
}

impl<D> CandidateArgumentMap<D> {
    /// Declaration-side types for exactly the inputs present at this call.
    /// Defaults contribute no source input; vararg elements use the element
    /// type while spread/named-array inputs retain the array parameter type.
    pub(crate) fn forwarding_parameter_types(
        &self,
        parameters: &[ValueParameter<D>],
    ) -> Vec<scoop_hir::TypeId> {
        self.source_order
            .iter()
            .map(|input| {
                let (parameter, kind) = self.source_binding(*input);
                let parameter = &parameters[parameter.index()];
                match (&parameter.calling, kind) {
                    (
                        ValueParameterCalling::Vararg { element_type, .. },
                        SourceInputKind::VarargElement,
                    ) => *element_type,
                    (ValueParameterCalling::Vararg { .. }, SourceInputKind::VarargArray)
                    | (
                        ValueParameterCalling::Required | ValueParameterCalling::Default(_),
                        SourceInputKind::Value,
                    ) => parameter.ty,
                    _ => unreachable!("argument mapping fixes each input shape"),
                }
            })
            .collect()
    }

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
