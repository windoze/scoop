use super::*;

/// Origin and passing convention of one physical Scoop ABI parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiPhysicalParameterOrigin {
    IndirectReturn,
    DirectArgument { logical_index: usize },
    DirectArgumentPart { logical_index: usize, part: AbiPart },
    IndirectArgument { logical_index: usize },
}

impl AbiPhysicalParameterOrigin {
    pub const fn logical_argument_index(self) -> Option<usize> {
        match self {
            Self::IndirectReturn => None,
            Self::DirectArgument { logical_index }
            | Self::DirectArgumentPart { logical_index, .. }
            | Self::IndirectArgument { logical_index } => Some(logical_index),
        }
    }
}

/// One computed entry in the physical parameter sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiPhysicalParameter<'a> {
    index: usize,
    origin: AbiPhysicalParameterOrigin,
    value: &'a AbiValue,
}

impl<'a> AbiPhysicalParameter<'a> {
    pub const fn index(self) -> usize {
        self.index
    }

    pub const fn origin(self) -> AbiPhysicalParameterOrigin {
        self.origin
    }

    pub const fn value(self) -> &'a AbiValue {
        self.value
    }
}

/// Computed physical location of one logical argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiArgumentLocation {
    Elided,
    Parameter(usize),
    Parts { first: usize, count: usize },
}

/// One Scoop calling signature with arguments retained in logical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoopAbiSignature {
    arguments: Vec<AbiArgument>,
    result: AbiReturn,
    calling_convention: CallingConvention,
}

impl ScoopAbiSignature {
    pub const fn new(
        arguments: Vec<AbiArgument>,
        result: AbiReturn,
        calling_convention: CallingConvention,
    ) -> Self {
        Self {
            arguments,
            result,
            calling_convention,
        }
    }

    pub fn arguments(&self) -> &[AbiArgument] {
        &self.arguments
    }

    pub const fn result(&self) -> &AbiReturn {
        &self.result
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }

    pub fn logical_argument_count(&self) -> usize {
        self.arguments.len()
    }

    pub fn physical_parameter_count(&self) -> usize {
        usize::from(self.result.is_indirect())
            + self
                .arguments
                .iter()
                .map(AbiArgument::parameter_count)
                .sum::<usize>()
    }

    pub fn argument_location(&self, logical_index: usize) -> Option<AbiArgumentLocation> {
        let argument = self.arguments.get(logical_index)?;
        if argument.is_elided() {
            return Some(AbiArgumentLocation::Elided);
        }

        let return_offset = usize::from(self.result.is_indirect());
        let preceding_parameters = self.arguments[..logical_index]
            .iter()
            .map(AbiArgument::parameter_count)
            .sum::<usize>();
        let first = return_offset + preceding_parameters;
        Some(match argument {
            AbiArgument::Direct(AbiDirectValue::DirectParts(parts)) => AbiArgumentLocation::Parts {
                first,
                count: parts.parts().len(),
            },
            _ => AbiArgumentLocation::Parameter(first),
        })
    }

    pub fn physical_parameters(&self) -> impl Iterator<Item = AbiPhysicalParameter<'_>> + '_ {
        let return_parameter = match &self.result {
            AbiReturn::Indirect(value) => Some((AbiPhysicalParameterOrigin::IndirectReturn, value)),
            AbiReturn::UnitVoid | AbiReturn::ElidedZst(_) | AbiReturn::Direct(_) => None,
        };
        let argument_parameters =
            self.arguments
                .iter()
                .enumerate()
                .flat_map(|(logical_index, argument)| {
                    (0..argument.parameter_count()).map(move |part_index| match argument {
                        AbiArgument::ElidedZst(_) => unreachable!("elided arguments have no parts"),
                        AbiArgument::Direct(AbiDirectValue::Scalar(value)) => (
                            AbiPhysicalParameterOrigin::DirectArgument { logical_index },
                            value,
                        ),
                        AbiArgument::Direct(AbiDirectValue::DirectParts(parts)) => (
                            AbiPhysicalParameterOrigin::DirectArgumentPart {
                                logical_index,
                                part: parts.parts()[part_index],
                            },
                            parts.value(),
                        ),
                        AbiArgument::Indirect(value) => (
                            AbiPhysicalParameterOrigin::IndirectArgument { logical_index },
                            value,
                        ),
                    })
                });

        return_parameter
            .into_iter()
            .chain(argument_parameters)
            .enumerate()
            .map(|(index, (origin, value))| AbiPhysicalParameter {
                index,
                origin,
                value,
            })
    }
}
