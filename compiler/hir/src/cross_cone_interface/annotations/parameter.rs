use super::wire::AnnotationDataResolver;
use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentAnnotationId, PersistentGenericTypeId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Annotation declarations are static schemas, not runtime nominal types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationParameterType<T> {
    Value(T),
    Annotation(PersistentAnnotationId),
    AnnotationArray {
        array: PersistentGenericTypeId,
        element: PersistentAnnotationId,
    },
}

impl<T> From<T> for AnnotationParameterType<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedAnnotationParameterTypeV1 {
    Value(DecodedSignatureTypeKey),
    Annotation(DecodedPersistentId<PersistentAnnotationId>),
    AnnotationArray {
        array: DecodedPersistentId<PersistentGenericTypeId>,
        element: DecodedPersistentId<PersistentAnnotationId>,
    },
}

impl DecodedAnnotationParameterTypeV1 {
    pub(super) fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotationParameterType<scoop_identity::SignatureTypeKey>, E> {
        Ok(match self {
            Self::Value(ty) => AnnotationParameterType::Value(ty.resolve(resolver)?),
            Self::Annotation(id) => AnnotationParameterType::Annotation(resolver.resolve(id)?),
            Self::AnnotationArray { array, element } => AnnotationParameterType::AnnotationArray {
                array: resolver.resolve(array)?,
                element: resolver.resolve(element)?,
            },
        })
    }
}

macro_rules! encode_parameter_type {
    () => {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(if matches!(self, Self::AnnotationArray { .. }) {
                3
            } else {
                2
            })?;
            encoder.field(0)?;
            encoder.unsigned(match self {
                Self::Value(_) => 1,
                Self::Annotation(_) => 2,
                Self::AnnotationArray { .. } => 3,
            })?;
            encoder.field(1)?;
            match self {
                Self::Value(ty) => ty.encode(encoder),
                Self::Annotation(id) => id.encode(encoder),
                Self::AnnotationArray { array, element } => {
                    array.encode(encoder)?;
                    encoder.field(2)?;
                    element.encode(encoder)
                }
            }
        }
    };
}
impl<T: WireEncode> WireEncode for AnnotationParameterType<T> {
    encode_parameter_type!();
}
impl WireEncode for DecodedAnnotationParameterTypeV1 {
    encode_parameter_type!();
}
impl WireDecode for DecodedAnnotationParameterTypeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        super::value::expect_fields(decoder, fields, if tag == 3 { 3 } else { 2 })?;
        match tag {
            1 => decoder
                .field(1, DecodedSignatureTypeKey::decode)
                .map(Self::Value),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Annotation),
            3 => Ok(Self::AnnotationArray {
                array: decoder.field(1, DecodedPersistentId::decode)?,
                element: decoder.field(2, DecodedPersistentId::decode)?,
            }),
            tag => Err(super::value::error(
                decoder,
                WireErrorKind::UnknownTag { tag },
            )),
        }
    }
}
