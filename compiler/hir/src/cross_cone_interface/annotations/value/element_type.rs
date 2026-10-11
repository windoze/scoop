use super::super::wire::AnnotationDataResolver;
use super::*;
use scoop_identity::{DecodedPersistentId, DecodedSignatureTypeKey};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnnotationElementTypeV1 {
    Scalar(CanonicalConstValueKindV1),
    IntegralEnum(SignatureTypeKey),
    Annotation(PersistentAnnotationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedAnnotationElementTypeV1 {
    Scalar(CanonicalConstValueKindV1),
    IntegralEnum(DecodedSignatureTypeKey),
    Annotation(DecodedPersistentId<PersistentAnnotationId>),
}

impl DecodedAnnotationElementTypeV1 {
    pub(super) fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotationElementTypeV1, E> {
        Ok(match self {
            Self::Scalar(kind) => AnnotationElementTypeV1::Scalar(kind),
            Self::IntegralEnum(ty) => AnnotationElementTypeV1::IntegralEnum(ty.resolve(resolver)?),
            Self::Annotation(id) => AnnotationElementTypeV1::Annotation(resolver.resolve(id)?),
        })
    }
}
macro_rules! encode_element_type {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(match self {
                    Self::Scalar(_) => 1,
                    Self::IntegralEnum(_) => 2,
                    Self::Annotation(_) => 3,
                })?;
                encoder.field(1)?;
                match self {
                    Self::Scalar(kind) => kind.encode(encoder),
                    Self::IntegralEnum(ty) => ty.encode(encoder),
                    Self::Annotation(id) => id.encode(encoder),
                }
            }
        }
    };
}
encode_element_type!(AnnotationElementTypeV1);
encode_element_type!(DecodedAnnotationElementTypeV1);
impl WireDecode for DecodedAnnotationElementTypeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, CanonicalConstValueKindV1::decode)
                .map(Self::Scalar),
            2 => decoder
                .field(1, DecodedSignatureTypeKey::decode)
                .map(Self::IntegralEnum),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Annotation),
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
