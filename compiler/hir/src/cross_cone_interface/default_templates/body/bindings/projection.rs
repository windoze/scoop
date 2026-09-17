use std::fmt;

use scoop_identity::{PersistentFieldId, SignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedDefaultFieldRefV1, DefaultFieldRefResolutionError, DefaultFieldRefV1,
    DefaultFieldReferenceResolver,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingProjectionV1(DefaultBindingProjectionKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum DefaultBindingProjectionKindV1 {
    TupleIndex(u32),
    StructField {
        declaration: PersistentFieldId,
        owner_type: SignatureTypeKey,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultBindingProjectionViewV1<'a> {
    TupleIndex(u32),
    StructField {
        declaration: PersistentFieldId,
        owner_type: &'a SignatureTypeKey,
    },
}

impl DefaultBindingProjectionV1 {
    pub const fn tuple_index(index: u32) -> Self {
        Self(DefaultBindingProjectionKindV1::TupleIndex(index))
    }

    pub fn struct_field(declaration: PersistentFieldId, owner_type: SignatureTypeKey) -> Self {
        Self(DefaultBindingProjectionKindV1::StructField {
            declaration,
            owner_type,
        })
    }

    pub fn try_struct_field(
        field: DefaultFieldRefV1,
    ) -> Result<Self, DefaultBindingProjectionBuildError> {
        match field {
            DefaultFieldRefV1::Struct {
                declaration,
                owner_type,
            } => Ok(Self::struct_field(declaration, owner_type)),
            DefaultFieldRefV1::Tuple { .. } | DefaultFieldRefV1::Class { .. } => {
                Err(DefaultBindingProjectionBuildError::ExpectedStructField)
            }
        }
    }

    pub fn view(&self) -> DefaultBindingProjectionViewV1<'_> {
        match &self.0 {
            DefaultBindingProjectionKindV1::TupleIndex(index) => {
                DefaultBindingProjectionViewV1::TupleIndex(*index)
            }
            DefaultBindingProjectionKindV1::StructField {
                declaration,
                owner_type,
            } => DefaultBindingProjectionViewV1::StructField {
                declaration: *declaration,
                owner_type,
            },
        }
    }
}

impl WireEncode for DefaultBindingProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match &self.0 {
            DefaultBindingProjectionKindV1::TupleIndex(_) => 1,
            DefaultBindingProjectionKindV1::StructField { .. } => 2,
        })?;
        encoder.field(1)?;
        match &self.0 {
            DefaultBindingProjectionKindV1::TupleIndex(index) => {
                encoder.unsigned(u64::from(*index))
            }
            DefaultBindingProjectionKindV1::StructField {
                declaration,
                owner_type,
            } => encode_struct_field_ref(encoder, declaration, owner_type),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultBindingProjectionV1 {
    TupleIndex(u32),
    StructField(DecodedDefaultFieldRefV1),
}

impl DecodedDefaultBindingProjectionV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultBindingProjectionV1, DefaultBindingProjectionResolutionError<E>>
    where
        R: DefaultFieldReferenceResolver<E>,
    {
        match self {
            Self::TupleIndex(index) => Ok(DefaultBindingProjectionV1::tuple_index(index)),
            Self::StructField(field) => DefaultBindingProjectionV1::try_struct_field(
                field
                    .resolve(resolver)
                    .map_err(DefaultBindingProjectionResolutionError::Field)?,
            )
            .map_err(DefaultBindingProjectionResolutionError::Shape),
        }
    }
}

impl WireEncode for DecodedDefaultBindingProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::TupleIndex(_) => 1,
            Self::StructField(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::TupleIndex(index) => encoder.unsigned(u64::from(*index)),
            Self::StructField(field) => field.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultBindingProjectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder.field(1, Decoder::u32).map(Self::TupleIndex),
            2 => decoder
                .field(1, DecodedDefaultFieldRefV1::decode)
                .map(Self::StructField),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingProjectionBuildError {
    ExpectedStructField,
}

impl fmt::Display for DefaultBindingProjectionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedStructField => {
                formatter.write_str("default binding projection requires a struct field")
            }
        }
    }
}

impl std::error::Error for DefaultBindingProjectionBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingProjectionResolutionError<E> {
    Field(DefaultFieldRefResolutionError<E>),
    Shape(DefaultBindingProjectionBuildError),
}

impl<E: fmt::Display> fmt::Display for DefaultBindingProjectionResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(error) => write!(formatter, "invalid binding projection field: {error}"),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultBindingProjectionResolutionError<E>
{
}

fn encode_struct_field_ref(
    encoder: &mut Encoder,
    declaration: &PersistentFieldId,
    owner_type: &SignatureTypeKey,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(0)?;
    encoder.unsigned(1)?;
    encoder.field(1)?;
    declaration.encode(encoder)?;
    encoder.field(2)?;
    owner_type.encode(encoder)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
