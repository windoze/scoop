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
use super::type_uses::{DecodedAnnotatedTypeUseV1, DecodedAnnotationTypeSiteV1};
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
    type_uses: Vec<DecodedAnnotatedTypeUseV1>,
    type_sites: Vec<DecodedAnnotationTypeSiteV1>,
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
                            value_type: parameter
                                .value_type
                                .resolve(resolver)
                                .map_err(AnnotationDataResolutionError::Reference)?,
                            default: parameter.default.map(|v| v.resolve(resolver)).transpose()?,
                        })
                    })
                    .collect::<Result<_, AnnotationDataResolutionError<E>>>()?;
                Ok(AnnotationDeclarationV1 {
                    annotation: resolver
                        .resolve(declaration.annotation)
                        .map_err(AnnotationDataResolutionError::Reference)?,
                    parameters,
                    rules: declaration.rules,
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
                            arguments: application
                                .arguments
                                .into_iter()
                                .map(|v| v.resolve(resolver))
                                .collect::<Result<_, _>>()?,
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
        let type_uses = self
            .type_uses
            .into_iter()
            .map(|usage| usage.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        let type_sites = self
            .type_sites
            .into_iter()
            .map(|site| site.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        if type_uses
            .windows(2)
            .any(|pair| pair[0].identity >= pair[1].identity)
            || type_sites
                .windows(2)
                .any(|pair| (pair[0].target, pair[0].slot) >= (pair[1].target, pair[1].slot))
            || declarations
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
        CanonicalAnnotationsV1::try_new(declarations, targets, type_uses, type_sites)
            .map_err(AnnotationDataResolutionError::Shape)
    }
}

macro_rules! encode_table {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(4)?;
                encoder.field(1)?;
                sequence(encoder, &self.declarations)?;
                encoder.field(2)?;
                sequence(encoder, &self.targets)?;
                encoder.field(3)?;
                sequence(encoder, &self.type_uses)?;
                encoder.field(4)?;
                sequence(encoder, &self.type_sites)
            }
        }
    };
}
encode_table!(CanonicalAnnotationsV1);
encode_table!(DecodedCanonicalAnnotationsV1);

impl WireDecode for DecodedCanonicalAnnotationsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            declarations: decoder.field(1, |d| {
                d.decode_array(|d, _| DecodedAnnotationDeclarationV1::decode(d))
            })?,
            targets: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedAnnotatedTargetV1::decode(d))
            })?,
            type_uses: decoder.field(3, |d| {
                d.decode_array(|d, _| DecodedAnnotatedTypeUseV1::decode(d))
            })?,
            type_sites: decoder.field(4, |d| {
                d.decode_array(|d, _| DecodedAnnotationTypeSiteV1::decode(d))
            })?,
        })
    }
}

pub(super) fn sequence<T: WireEncode>(
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
    Source(scoop_identity::SourceIdentityResolutionError<E>),
    Span(scoop_identity::SourceSpanError),
    Shape(AnnotationDataBuildError),
}
impl<E: fmt::Display> fmt::Display for AnnotationDataResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Name(error) => error.fmt(formatter),
            Self::Origin(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Span(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for AnnotationDataResolutionError<E> {}
