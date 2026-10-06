use super::*;
use crate::ExternalHirTargetResolver;
use scoop_identity::{
    CanonicalIdentifierError, ConeIdentity, PersistentAnnotationId, PersistentIdResolver,
    PersistentKeyResolver, PersistentSourceContextId, SourceContextKey,
    SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};
use std::fmt;

mod records;
use records::{DecodedAnnotatedTargetV1, DecodedAnnotationDeclarationV1};

pub trait AnnotationDataResolver<E>:
    ExternalHirTargetResolver<E>
    + PersistentIdResolver<PersistentAnnotationId, Error = E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}
impl<R, E> AnnotationDataResolver<E> for R where
    R: ExternalHirTargetResolver<E>
        + PersistentIdResolver<PersistentAnnotationId, Error = E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalAnnotationsV1 {
    declarations: Vec<DecodedAnnotationDeclarationV1>,
    targets: Vec<DecodedAnnotatedTargetV1>,
}

impl DecodedCanonicalAnnotationsV1 {
    pub fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalAnnotationsV1, AnnotationDataResolutionError<E>> {
        let declarations = self
            .declarations
            .into_iter()
            .map(|declaration| {
                let parameters = declaration
                    .parameters
                    .into_iter()
                    .map(|parameter| {
                        Ok(AnnotationParameterV1 {
                            name: parameter
                                .name
                                .validate()
                                .map_err(AnnotationDataResolutionError::Name)?,
                            value_type: resolver
                                .resolve(parameter.value_type)
                                .map_err(AnnotationDataResolutionError::Reference)?,
                            default: parameter.default,
                        })
                    })
                    .collect::<Result<_, AnnotationDataResolutionError<E>>>()?;
                Ok(AnnotationDeclarationV1 {
                    annotation: resolver
                        .resolve(declaration.annotation)
                        .map_err(AnnotationDataResolutionError::Reference)?,
                    parameters,
                    visibility: declaration.visibility,
                    definition_origin: declaration
                        .definition_origin
                        .resolve(resolver)
                        .map_err(AnnotationDataResolutionError::Origin)?,
                })
            })
            .collect::<Result<Vec<_>, AnnotationDataResolutionError<E>>>()?;
        let targets = self
            .targets
            .into_iter()
            .map(|target| {
                let annotations = target
                    .annotations
                    .into_iter()
                    .map(|application| {
                        Ok(AnnotationApplicationV1 {
                            annotation: resolver
                                .resolve(application.annotation)
                                .map_err(AnnotationDataResolutionError::Reference)?,
                            arguments: application.arguments,
                            definition_origin: application
                                .definition_origin
                                .resolve(resolver)
                                .map_err(AnnotationDataResolutionError::Origin)?,
                        })
                    })
                    .collect::<Result<_, AnnotationDataResolutionError<E>>>()?;
                Ok(AnnotatedTargetV1 {
                    target: target
                        .target
                        .resolve(resolver)
                        .map_err(AnnotationDataResolutionError::Reference)?,
                    annotations,
                })
            })
            .collect::<Result<Vec<_>, AnnotationDataResolutionError<E>>>()?;
        if declarations
            .windows(2)
            .any(|pair| pair[0].annotation >= pair[1].annotation)
            || targets
                .windows(2)
                .any(|pair| pair[0].target >= pair[1].target)
        {
            return Err(AnnotationDataResolutionError::Shape(
                AnnotationDataBuildError("annotation table is not in canonical order"),
            ));
        }
        CanonicalAnnotationsV1::try_new(declarations, targets)
            .map_err(AnnotationDataResolutionError::Shape)
    }
}

macro_rules! encode_table {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                sequence(encoder, &self.declarations)?;
                encoder.field(2)?;
                sequence(encoder, &self.targets)
            }
        }
    };
}
encode_table!(CanonicalAnnotationsV1);
encode_table!(DecodedCanonicalAnnotationsV1);

impl WireDecode for DecodedCanonicalAnnotationsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            declarations: decoder.field(1, |d| {
                d.decode_array(|d, _| DecodedAnnotationDeclarationV1::decode(d))
            })?,
            targets: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedAnnotatedTargetV1::decode(d))
            })?,
        })
    }
}

fn sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum AnnotationDataResolutionError<E> {
    Reference(E),
    Name(CanonicalIdentifierError),
    Origin(SourceOriginResolutionError<E>),
    Shape(AnnotationDataBuildError),
}
impl<E: fmt::Display> fmt::Display for AnnotationDataResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Name(error) => error.fmt(formatter),
            Self::Origin(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for AnnotationDataResolutionError<E> {}
