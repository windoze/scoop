use super::*;
use crate::{DecodedDefaultRequirementBindingV1, DefaultRequirementBindingV1};

/// A lexical substitution retains the requirements checked at its creation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCallableBodyTypeArgumentsV1 {
    arguments: Option<Vec<SignatureTypeKey>>,
    requirements: Vec<DefaultRequirementBindingV1>,
}

impl DefaultCallableBodyTypeArgumentsV1 {
    pub const fn lexical() -> Self {
        Self {
            arguments: None,
            requirements: Vec::new(),
        }
    }
    pub fn try_explicit(
        arguments: Vec<SignatureTypeKey>,
    ) -> Result<Self, DefaultCallableBodyTypeArgumentsBuildError> {
        u32::try_from(arguments.len())
            .map_err(|_| DefaultCallableBodyTypeArgumentsBuildError::TooManyArguments)?;
        Ok(Self {
            arguments: Some(arguments),
            requirements: Vec::new(),
        })
    }
    pub fn with_requirements(mut self, requirements: Vec<DefaultRequirementBindingV1>) -> Self {
        self.requirements = requirements;
        self
    }
    pub fn requirements(&self) -> &[DefaultRequirementBindingV1] {
        &self.requirements
    }
    pub const fn is_lexical(&self) -> bool {
        self.arguments.is_none()
    }
    pub fn explicit_arguments(&self) -> Option<&[SignatureTypeKey]> {
        self.arguments.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCallableBodyTypeArgumentsV1 {
    arguments: Option<Vec<DecodedSignatureTypeKey>>,
    requirements: Vec<DecodedDefaultRequirementBindingV1>,
}

impl DecodedDefaultCallableBodyTypeArgumentsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<
        DefaultCallableBodyTypeArgumentsV1,
        DefaultCallableBodyTypeArgumentsResolutionError<E>,
    >
    where
        R: crate::DefaultCallableReferenceResolver<E>,
    {
        use DefaultCallableBodyTypeArgumentsResolutionError as Error;
        if let Some(arguments) = &self.arguments {
            u32::try_from(arguments.len()).map_err(|_| Error::TooManyArguments)?;
        }
        let arguments = self
            .arguments
            .map(|arguments| {
                arguments
                    .into_iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        argument
                            .resolve(resolver)
                            .map_err(|error| Error::Argument { index, error })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let requirements = self
            .requirements
            .into_iter()
            .map(|binding| {
                binding
                    .resolve(resolver)
                    .map_err(|error| Error::Requirement(Box::new(error)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DefaultCallableBodyTypeArgumentsV1 {
            arguments,
            requirements,
        })
    }
}

impl WireEncode for DefaultCallableBodyTypeArgumentsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_arguments(encoder, self.arguments.as_deref(), &self.requirements)
    }
}
impl WireEncode for DecodedDefaultCallableBodyTypeArgumentsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_arguments(encoder, self.arguments.as_deref(), &self.requirements)
    }
}
fn encode_arguments(
    encoder: &mut Encoder,
    arguments: Option<&[impl WireEncode]>,
    requirements: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1 + u64::from(arguments.is_some()) + u64::from(!requirements.is_empty()))?;
    encode_tag(encoder, if arguments.is_some() { 2 } else { 1 })?;
    if let Some(arguments) = arguments {
        encoder.field(1)?;
        encode_sequence(encoder, arguments)?;
    }
    if !requirements.is_empty() {
        encoder.field(2)?;
        encode_sequence(encoder, requirements)?;
    }
    Ok(())
}
impl WireDecode for DecodedDefaultCallableBodyTypeArgumentsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let base = match tag {
            1 => 1,
            2 => 2,
            tag => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if fields != base && fields != base + 1 {
            return Err(wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: base,
                    actual: fields,
                },
            ));
        }
        let arguments = if tag == 2 {
            Some(decoder.field(1, |d| {
                d.decode_array(|d, _| DecodedSignatureTypeKey::decode(d))
            })?)
        } else {
            None
        };
        let requirements = if fields == base + 1 {
            let values = decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedDefaultRequirementBindingV1::decode(d))
            })?;
            if values.is_empty() {
                return Err(wire_error(decoder, WireErrorKind::NonCanonicalCbor));
            }
            values
        } else {
            Vec::new()
        };
        Ok(Self {
            arguments,
            requirements,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultCallableBodyTypeArgumentsBuildError {
    TooManyArguments,
}

impl fmt::Display for DefaultCallableBodyTypeArgumentsBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyArguments => {
                formatter.write_str("default callable body type argument count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultCallableBodyTypeArgumentsBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCallableBodyTypeArgumentsResolutionError<E> {
    TooManyArguments,
    Argument { index: usize, error: E },
    Requirement(Box<crate::RequirementBindingResolutionError<E>>),
}

impl<E: fmt::Display> fmt::Display for DefaultCallableBodyTypeArgumentsResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyArguments => {
                formatter.write_str("default callable body type argument count exceeds u32")
            }
            Self::Requirement(error) => {
                write!(formatter, "invalid lexical requirement binding: {error}")
            }
            Self::Argument { index, error } => {
                write!(
                    formatter,
                    "invalid default callable body type argument {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultCallableBodyTypeArgumentsResolutionError<E>
{
}
