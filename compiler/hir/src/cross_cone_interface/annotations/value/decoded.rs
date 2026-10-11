use super::super::wire::{AnnotationDataResolutionError as Error, AnnotationDataResolver};
use super::element_type::DecodedAnnotationElementTypeV1;
use super::*;
use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, DecodedSourceIdentity, DecodedSourceSpan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedAnnotationValueV1 {
    Scalar(CanonicalConstValueV1),
    IntegralEnum {
        value_type: DecodedSignatureTypeKey,
        variant: DecodedPersistentId<PersistentEnumVariantId>,
        value: CanonicalIntegerConstantV1,
    },
    Annotation {
        annotation: DecodedPersistentId<PersistentAnnotationId>,
        arguments: Vec<Self>,
    },
    Array {
        element_type: DecodedAnnotationElementTypeV1,
        elements: Vec<DecodedAnnotationElementV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedAnnotationElementV1 {
    pub(super) value: DecodedAnnotationValueV1,
    pub(super) source: DecodedSourceIdentity,
    pub(super) span: DecodedSourceSpan,
}

impl DecodedAnnotationValueV1 {
    pub fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalAnnotationValueV1, Error<E>> {
        Ok(match self {
            Self::Scalar(value) => CanonicalAnnotationValueV1::Scalar(value),
            Self::IntegralEnum {
                value_type,
                variant,
                value,
            } => CanonicalAnnotationValueV1::IntegralEnum {
                value_type: value_type.resolve(resolver).map_err(Error::Reference)?,
                variant: resolver.resolve(variant).map_err(Error::Reference)?,
                value,
            },
            Self::Annotation {
                annotation,
                arguments,
            } => CanonicalAnnotationValueV1::Annotation {
                annotation: resolver.resolve(annotation).map_err(Error::Reference)?,
                arguments: arguments
                    .into_iter()
                    .map(|v| v.resolve(resolver))
                    .collect::<Result<_, _>>()?,
            },
            Self::Array {
                element_type,
                elements,
            } => CanonicalAnnotationValueV1::Array {
                element_type: element_type.resolve(resolver).map_err(Error::Reference)?,
                elements: elements
                    .into_iter()
                    .map(|e| {
                        Ok(AnnotationElementV1 {
                            value: e.value.resolve(resolver)?,
                            source: e.source.resolve(resolver).map_err(Error::Source)?,
                            span: e.span.validate().map_err(Error::Span)?,
                        })
                    })
                    .collect::<Result<_, Error<E>>>()?,
            },
        })
    }
}

impl WireDecode for DecodedAnnotationValueV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, CanonicalConstValueV1::decode)
                    .map(Self::Scalar)
            }
            2 => {
                expect_fields(decoder, fields, 3)?;
                Ok(Self::Array {
                    element_type: decoder.field(1, DecodedAnnotationElementTypeV1::decode)?,
                    elements: decoder.field(2, |d| {
                        d.decode_array(|d, _| DecodedAnnotationElementV1::decode(d))
                    })?,
                })
            }
            3 => {
                expect_fields(decoder, fields, 4)?;
                Ok(Self::IntegralEnum {
                    value_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    variant: decoder.field(2, DecodedPersistentId::decode)?,
                    value: decoder.field(3, CanonicalIntegerConstantV1::decode)?,
                })
            }
            4 => {
                expect_fields(decoder, fields, 3)?;
                Ok(Self::Annotation {
                    annotation: decoder.field(1, DecodedPersistentId::decode)?,
                    arguments: decoder.field(2, |d| d.decode_array(|d, _| Self::decode(d)))?,
                })
            }
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireDecode for DecodedAnnotationElementV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            value: decoder.field(1, DecodedAnnotationValueV1::decode)?,
            source: decoder.field(2, DecodedSourceIdentity::decode)?,
            span: decoder.field(3, DecodedSourceSpan::decode)?,
        })
    }
}
