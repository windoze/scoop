use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    ConcreteExpressionOrigin, DefinitionOrigin, EvaluationOrigin, ExpressionOrigin,
    SourceContextKey, SourceOriginError, SourceSpan, SourceSpanError,
};
use crate::{
    ConeIdentity, DecodedCallableOwner, DecodedNominalDeclarationOwner, DecodedPersistentId,
    DecodedPropertyOwner, DecodedSourceIdentity, PersistentCallableApplicationId,
    PersistentConstructorId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSourceContextId,
    PersistentTypeId, SourceIdentityResolutionError,
};

mod subject;

pub use subject::{
    DecodedDefinitionOriginRecord, DecodedDefinitionOriginSubject,
    DefinitionOriginRecordResolutionError, DefinitionOriginSubjectResolver,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceContextKey {
    File {
        source: DecodedSourceIdentity,
    },
    Nominal {
        source: DecodedSourceIdentity,
        owner: DecodedNominalDeclarationOwner,
    },
    Callable {
        source: DecodedSourceIdentity,
        owner: DecodedCallableOwner,
    },
    Property {
        source: DecodedSourceIdentity,
        owner: DecodedPropertyOwner,
    },
    Initialization {
        source: DecodedSourceIdentity,
        unit: DecodedPersistentId<PersistentInitializationUnitId>,
    },
}

impl DecodedSourceContextKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceContextKey, SourceContextResolutionError<E>>
    where
        R: SourceContextResolver<E>,
    {
        match self {
            Self::File { source } => {
                resolve_source(source, resolver).map(|source| SourceContextKey::File { source })
            }
            Self::Nominal { source, owner } => {
                let source = resolve_source(source, resolver)?;
                let owner = owner
                    .resolve(resolver)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Nominal { source, owner })
            }
            Self::Callable { source, owner } => {
                let source = resolve_source(source, resolver)?;
                let owner = owner
                    .resolve(resolver)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Callable { source, owner })
            }
            Self::Property { source, owner } => {
                let source = resolve_source(source, resolver)?;
                let owner = owner
                    .resolve(resolver)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Property { source, owner })
            }
            Self::Initialization { source, unit } => {
                let source = resolve_source(source, resolver)?;
                let unit = resolver
                    .resolve(unit)
                    .map_err(SourceContextResolutionError::Reference)?;
                Ok(SourceContextKey::Initialization { source, unit })
            }
        }
    }
}

impl WireEncode for DecodedSourceContextKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::File { source } => encode_context(encoder, 1, source, None),
            Self::Nominal { source, owner } => encode_context(encoder, 2, source, Some(owner)),
            Self::Callable { source, owner } => encode_context(encoder, 3, source, Some(owner)),
            Self::Property { source, owner } => encode_context(encoder, 4, source, Some(owner)),
            Self::Initialization { source, unit } => encode_context(encoder, 5, source, Some(unit)),
        }
    }
}

impl WireDecode for DecodedSourceContextKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSourceIdentity::decode)
                    .map(|source| Self::File { source })
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Nominal {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    owner: decoder.field(2, DecodedNominalDeclarationOwner::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Callable {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    owner: decoder.field(2, DecodedCallableOwner::decode)?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Property {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    owner: decoder.field(2, DecodedPropertyOwner::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Initialization {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    unit: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedSourceSpan {
    start_byte: u64,
    end_byte: u64,
}

impl DecodedSourceSpan {
    pub fn validate(self) -> Result<SourceSpan, SourceSpanError> {
        SourceSpan::new(self.start_byte, self.end_byte)
    }
}

impl WireEncode for DecodedSourceSpan {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(self.start_byte)?;
        encoder.field(2)?;
        encoder.unsigned(self.end_byte)
    }
}

impl WireDecode for DecodedSourceSpan {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            start_byte: decoder.field(1, Decoder::unsigned)?,
            end_byte: decoder.field(2, Decoder::unsigned)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefinitionOrigin {
    source: DecodedSourceIdentity,
    span: DecodedSourceSpan,
    context: DecodedPersistentId<PersistentSourceContextId>,
}

impl DecodedDefinitionOrigin {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefinitionOrigin, SourceOriginResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        let (source, span, context) =
            resolve_origin_fields(self.source, self.span, self.context, resolver)?;
        DefinitionOrigin::new(source, span, &context).map_err(SourceOriginResolutionError::Origin)
    }
}

impl WireEncode for DecodedDefinitionOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_origin(encoder, &self.source, self.span, self.context)
    }
}

impl WireDecode for DecodedDefinitionOrigin {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            source: decoder.field(1, DecodedSourceIdentity::decode)?,
            span: decoder.field(2, DecodedSourceSpan::decode)?,
            context: decoder.field(3, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEvaluationOrigin {
    source: DecodedSourceIdentity,
    span: DecodedSourceSpan,
    context: DecodedPersistentId<PersistentSourceContextId>,
}

impl DecodedEvaluationOrigin {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<EvaluationOrigin, SourceOriginResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        let (source, span, context) =
            resolve_origin_fields(self.source, self.span, self.context, resolver)?;
        EvaluationOrigin::new(source, span, &context).map_err(SourceOriginResolutionError::Origin)
    }
}

impl WireEncode for DecodedEvaluationOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_origin(encoder, &self.source, self.span, self.context)
    }
}

impl WireDecode for DecodedEvaluationOrigin {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            source: decoder.field(1, DecodedSourceIdentity::decode)?,
            span: decoder.field(2, DecodedSourceSpan::decode)?,
            context: decoder.field(3, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedConcreteExpressionOrigin {
    definition: DecodedDefinitionOrigin,
    evaluation: DecodedEvaluationOrigin,
}

impl DecodedConcreteExpressionOrigin {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ConcreteExpressionOrigin, SourceOriginResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        let definition = self.definition.resolve(resolver)?;
        let evaluation = self.evaluation.resolve(resolver)?;
        Ok(ConcreteExpressionOrigin::new(definition, evaluation))
    }
}

impl WireEncode for DecodedConcreteExpressionOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.evaluation.encode(encoder)
    }
}

impl WireDecode for DecodedConcreteExpressionOrigin {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            definition: decoder.field(1, DecodedDefinitionOrigin::decode)?,
            evaluation: decoder.field(2, DecodedEvaluationOrigin::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedExpressionOrigin {
    Definition(DecodedDefinitionOrigin),
    Concrete(DecodedConcreteExpressionOrigin),
}

impl DecodedExpressionOrigin {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExpressionOrigin, SourceOriginResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        match self {
            Self::Definition(origin) => origin.resolve(resolver).map(ExpressionOrigin::Definition),
            Self::Concrete(origin) => origin.resolve(resolver).map(ExpressionOrigin::Concrete),
        }
    }
}

impl WireEncode for DecodedExpressionOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Definition(_) => 1,
            Self::Concrete(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Definition(origin) => origin.encode(encoder),
            Self::Concrete(origin) => origin.encode(encoder),
        }
    }
}

impl WireDecode for DecodedExpressionOrigin {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => decoder
                .field(1, DecodedDefinitionOrigin::decode)
                .map(Self::Definition),
            2 => decoder
                .field(1, DecodedConcreteExpressionOrigin::decode)
                .map(Self::Concrete),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceOriginResolutionError<E> {
    Source(SourceIdentityResolutionError<E>),
    Span(SourceSpanError),
    Context(E),
    Origin(SourceOriginError),
}

impl<E: fmt::Display> fmt::Display for SourceOriginResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Span(error) => error.fmt(formatter),
            Self::Context(error) => error.fmt(formatter),
            Self::Origin(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceOriginResolutionError<E> {}

pub trait SourceContextResolver<E>:
    PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentTypeId, Error = E>
    + PersistentIdResolver<PersistentGenericTypeId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

impl<R, E> SourceContextResolver<E> for R where
    R: PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceContextResolutionError<E> {
    Source(SourceIdentityResolutionError<E>),
    Reference(E),
}

impl<E: fmt::Display> fmt::Display for SourceContextResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Reference(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceContextResolutionError<E> {}

fn resolve_source<R, E>(
    source: DecodedSourceIdentity,
    resolver: &mut R,
) -> Result<crate::SourceIdentity, SourceContextResolutionError<E>>
where
    R: PersistentIdResolver<ConeIdentity, Error = E>,
{
    source
        .resolve(resolver)
        .map_err(SourceContextResolutionError::Source)
}

fn resolve_origin_fields<R, E>(
    source: DecodedSourceIdentity,
    span: DecodedSourceSpan,
    context: DecodedPersistentId<PersistentSourceContextId>,
    resolver: &mut R,
) -> Result<(crate::SourceIdentity, SourceSpan, SourceContextKey), SourceOriginResolutionError<E>>
where
    R: PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
{
    let source = source
        .resolve(resolver)
        .map_err(SourceOriginResolutionError::Source)?;
    let span = span.validate().map_err(SourceOriginResolutionError::Span)?;
    let context = resolver
        .resolve_key(context)
        .map_err(SourceOriginResolutionError::Context)?;
    Ok((source, span, context))
}

fn encode_origin(
    encoder: &mut Encoder,
    source: &DecodedSourceIdentity,
    span: DecodedSourceSpan,
    context: DecodedPersistentId<PersistentSourceContextId>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    span.encode(encoder)?;
    encoder.field(3)?;
    context.encode(encoder)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_context(
    encoder: &mut Encoder,
    tag: u64,
    source: &DecodedSourceIdentity,
    owner: Option<&dyn WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(if owner.is_some() { 3 } else { 2 })?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    if let Some(owner) = owner {
        encoder.field(2)?;
        owner.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
