use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, EnumVariantFieldKey, FieldIdentityKey,
    PersistentEnumVariantFieldId, PersistentFieldId, PersistentId, PersistentIdMismatch,
    PersistentKeyResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::SignatureTypeReferenceResolver;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedField<I: PersistentId> {
    field: DecodedPersistentId<I>,
    value_type: DecodedSignatureTypeKey,
}

impl<I: PersistentId> WireDecode for DecodedField<I> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}
impl<I: PersistentId> WireEncode for DecodedField<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

macro_rules! decoded_field {
    ($decoded:ident, $resolved:ident, $id:ty, $key:ty) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $decoded(DecodedField<$id>);
        impl $decoded {
            pub fn resolve<R, E>(
                self,
                resolver: &mut R,
            ) -> Result<$resolved, RepresentationFieldResolutionError<E, $id>>
            where
                R: SignatureTypeReferenceResolver<E> + PersistentKeyResolver<$id, $key, Error = E>,
            {
                let key = resolver
                    .resolve_key(self.0.field)
                    .map_err(RepresentationFieldResolutionError::Key)?;
                let value_type = self
                    .0
                    .value_type
                    .resolve(resolver)
                    .map_err(RepresentationFieldResolutionError::Type)?;
                let record = $resolved::try_new(&key, value_type)
                    .map_err(RepresentationFieldResolutionError::Field)?;
                self.0
                    .field
                    .verify(record.field())
                    .map_err(RepresentationFieldResolutionError::Identity)?;
                Ok(record)
            }
        }
        impl WireDecode for $decoded {
            fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
                DecodedField::decode(decoder).map(Self)
            }
        }
        impl WireEncode for $decoded {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                self.0.encode(encoder)
            }
        }
    };
}
decoded_field!(
    DecodedStructRepresentationFieldV1,
    StructRepresentationFieldV1,
    PersistentFieldId,
    FieldIdentityKey
);
decoded_field!(
    DecodedClassRepresentationFieldV1,
    ClassRepresentationFieldV1,
    PersistentFieldId,
    FieldIdentityKey
);
decoded_field!(
    DecodedEnumRepresentationFieldV1,
    EnumRepresentationFieldV1,
    PersistentEnumVariantFieldId,
    EnumVariantFieldKey
);

#[derive(Debug)]
pub enum RepresentationFieldResolutionError<E, I: PersistentId> {
    Key(E),
    Type(E),
    Field(RepresentationFieldBuildError),
    Identity(PersistentIdMismatch<I>),
}
impl<E: fmt::Display, I: PersistentId> fmt::Display for RepresentationFieldResolutionError<E, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(error) => write!(f, "invalid representation field key: {error}"),
            Self::Type(error) => write!(f, "invalid representation field type: {error}"),
            Self::Field(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static, I: PersistentId> std::error::Error
    for RepresentationFieldResolutionError<E, I>
{
}
