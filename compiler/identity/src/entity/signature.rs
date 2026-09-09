use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use crate::{PersistentGenericTypeId, PersistentTypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonEmptyVecError;

impl fmt::Display for NonEmptyVecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a non-empty sequence must contain at least one value")
    }
}

impl std::error::Error for NonEmptyVecError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    pub fn new(values: Vec<T>) -> Result<Self, NonEmptyVecError> {
        if values.is_empty() {
            Err(NonEmptyVecError)
        } else {
            Ok(Self(values))
        }
    }

    pub fn from_first(first: T, rest: impl IntoIterator<Item = T>) -> Self {
        let mut values = vec![first];
        values.extend(rest);
        Self(values)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Effect {
    Ordinary,
    Suspend,
}

impl WireEncode for Effect {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Ordinary => 1,
            Self::Suspend => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallingConvention {
    C,
}

impl WireEncode for CallingConvention {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignatureTypeKey {
    Nominal(PersistentTypeId),
    NominalApplication {
        origin: PersistentGenericTypeId,
        arguments: NonEmptyVec<SignatureTypeKey>,
    },
    Tuple(NonEmptyVec<SignatureTypeKey>),
    Function {
        effect: Effect,
        parameters: Vec<SignatureTypeKey>,
        result: Box<SignatureTypeKey>,
    },
    RawPointer(Box<SignatureTypeKey>),
    NativeFunctionPointer {
        calling_convention: CallingConvention,
        parameters: Vec<SignatureTypeKey>,
        result: Box<SignatureTypeKey>,
    },
    Binder {
        depth: u32,
        index: u32,
    },
}

impl WireEncode for SignatureTypeKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(id) => encode_single_payload(encoder, 1, id),
            Self::NominalApplication { origin, arguments } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                origin.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, arguments.as_slice())
            }
            Self::Tuple(elements) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, elements.as_slice())
            }
            Self::Function {
                effect,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                effect.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
            Self::RawPointer(pointee) => encode_single_payload(encoder, 5, pointee.as_ref()),
            Self::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                calling_convention.encode(encoder)?;
                encoder.field(2)?;
                encode_sequence(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
            Self::Binder { depth, index } => {
                encoder.map(3)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*depth))?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*index))
            }
        }
    }
}

impl SignatureTypeKey {
    pub fn contains_binder(&self) -> bool {
        match self {
            Self::Nominal(_) => false,
            Self::NominalApplication { arguments, .. } | Self::Tuple(arguments) => {
                arguments.as_slice().iter().any(Self::contains_binder)
            }
            Self::Function {
                parameters, result, ..
            }
            | Self::NativeFunctionPointer {
                parameters, result, ..
            } => parameters.iter().any(Self::contains_binder) || result.contains_binder(),
            Self::RawPointer(pointee) => pointee.contains_binder(),
            Self::Binder { .. } => true,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalSignatureType {
    Absent,
    Present(Box<SignatureTypeKey>),
}

impl OptionalSignatureType {
    pub fn from_option(value: Option<SignatureTypeKey>) -> Self {
        match value {
            Some(value) => Self::Present(Box::new(value)),
            None => Self::Absent,
        }
    }

    pub fn is_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
}

impl WireEncode for OptionalSignatureType {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            Self::Present(value) => encode_single_payload(encoder, 2, value.as_ref()),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DuplicateSignatureKey {
    Nominal {
        type_parameter_count: u32,
    },
    Function {
        type_parameter_count: u32,
        receiver: OptionalSignatureType,
        parameters: Vec<SignatureTypeKey>,
    },
    Constructor {
        parameters: Vec<SignatureTypeKey>,
    },
    Property {
        type_parameter_count: u32,
        receiver: OptionalSignatureType,
    },
    TypeAlias,
}

impl DuplicateSignatureKey {
    pub fn type_parameter_count(&self) -> u32 {
        match self {
            Self::Nominal {
                type_parameter_count,
            }
            | Self::Function {
                type_parameter_count,
                ..
            }
            | Self::Property {
                type_parameter_count,
                ..
            } => *type_parameter_count,
            Self::Constructor { .. } | Self::TypeAlias => 0,
        }
    }

    pub fn receiver_is_present(&self) -> bool {
        match self {
            Self::Function { receiver, .. } | Self::Property { receiver, .. } => {
                receiver.is_present()
            }
            Self::Nominal { .. } | Self::Constructor { .. } | Self::TypeAlias => false,
        }
    }
}

impl WireEncode for DuplicateSignatureKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal {
                type_parameter_count,
            } => {
                encoder.map(2)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))
            }
            Self::Function {
                type_parameter_count,
                receiver,
                parameters,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))?;
                encoder.field(2)?;
                receiver.encode(encoder)?;
                encoder.field(3)?;
                encode_sequence(encoder, parameters)
            }
            Self::Constructor { parameters } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_sequence(encoder, parameters)
            }
            Self::Property {
                type_parameter_count,
                receiver,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))?;
                encoder.field(2)?;
                receiver.encode(encoder)
            }
            Self::TypeAlias => {
                encoder.map(1)?;
                encode_tag(encoder, 5)
            }
        }
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_single_payload(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        DuplicateSignatureKey, Effect, NonEmptyVec, OptionalSignatureType, SignatureTypeKey,
    };

    #[test]
    fn non_empty_signature_collections_are_structural() {
        assert!(NonEmptyVec::<SignatureTypeKey>::new(Vec::new()).is_err());
    }

    #[test]
    fn signature_binder_and_function_have_fixed_wire() {
        let binder = SignatureTypeKey::Binder { depth: 1, index: 2 };
        assert_eq!(encode(&binder).unwrap(), b"\xa3\x00\x07\x01\x01\x02\x02");
        let function = SignatureTypeKey::Function {
            effect: Effect::Suspend,
            parameters: vec![binder.clone()],
            result: Box::new(binder),
        };
        assert_eq!(
            encode(&function).unwrap(),
            b"\xa4\x00\x04\x01\x02\x02\x81\xa3\x00\x07\x01\x01\x02\x02\x03\xa3\x00\x07\x01\x01\x02\x02"
        );
    }

    #[test]
    fn absent_receiver_is_an_explicit_sum() {
        let signature = DuplicateSignatureKey::Property {
            type_parameter_count: 0,
            receiver: OptionalSignatureType::Absent,
        };
        assert_eq!(
            encode(&signature).unwrap(),
            b"\xa3\x00\x04\x01\x00\x02\xa1\x00\x01"
        );
    }
}
