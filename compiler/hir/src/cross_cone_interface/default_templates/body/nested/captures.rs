use std::fmt;

use scoop_identity::{
    DecodedSignatureTypeKey, LocalValueSelector, SignatureTypeKey, SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedExportDefinitionSourceV1, ExportDefinitionSourceV1, TemplateLocalIndexResolver,
    TemplateLocalReferenceResolver, TemplateLocalSelectorResolver,
};

/// One hidden closure-environment input in provider ABI order.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCaptureV1 {
    source: LocalValueSelector,
    value_type: SignatureTypeKey,
    first_use_origin: ExportDefinitionSourceV1,
}

impl DefaultCaptureV1 {
    pub const fn new(
        source: LocalValueSelector,
        value_type: SignatureTypeKey,
        first_use_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            source,
            value_type,
            first_use_origin,
        }
    }

    pub const fn source(&self) -> &LocalValueSelector {
        &self.source
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn first_use_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.first_use_origin
    }

    pub fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultCaptureV1<'_>, DefaultCaptureIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let source_index = resolver
            .resolve_template_local_index(&self.source)
            .map_err(DefaultCaptureIndexError::Source)?;
        Ok(IndexedDefaultCaptureV1 {
            capture: self,
            source_index,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCaptureV1 {
    source_index: u32,
    value_type: DecodedSignatureTypeKey,
    first_use_origin: DecodedExportDefinitionSourceV1,
}

impl DecodedDefaultCaptureV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultCaptureV1, DefaultCaptureResolutionError<E, L::Error>>
    where
        R: TemplateLocalReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        Ok(DefaultCaptureV1 {
            source: locals
                .resolve_template_local_selector(self.source_index)
                .map_err(DefaultCaptureResolutionError::Source)?,
            value_type: self
                .value_type
                .resolve(resolver)
                .map_err(DefaultCaptureResolutionError::ValueType)?,
            first_use_origin: self
                .first_use_origin
                .resolve(resolver)
                .map_err(DefaultCaptureResolutionError::FirstUseOrigin)?,
        })
    }
}

impl WireEncode for DecodedDefaultCaptureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.source_index))?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.first_use_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultCaptureV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            source_index: decoder.field(1, Decoder::u32)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            first_use_origin: decoder.field(3, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct IndexedDefaultCaptureV1<'a> {
    capture: &'a DefaultCaptureV1,
    source_index: u32,
}

impl WireEncode for IndexedDefaultCaptureV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.source_index))?;
        encoder.field(2)?;
        self.capture.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.capture.first_use_origin.encode(encoder)
    }
}

/// Whether a nested body inherits the lexical binder stack or receives an
/// explicit declaration-order substitution.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCallableBodyTypeArgumentsV1(DefaultCallableBodyTypeArgumentsKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum DefaultCallableBodyTypeArgumentsKindV1 {
    Lexical,
    Explicit(Vec<SignatureTypeKey>),
}

impl DefaultCallableBodyTypeArgumentsV1 {
    pub const fn lexical() -> Self {
        Self(DefaultCallableBodyTypeArgumentsKindV1::Lexical)
    }

    pub fn try_explicit(
        arguments: Vec<SignatureTypeKey>,
    ) -> Result<Self, DefaultCallableBodyTypeArgumentsBuildError> {
        u32::try_from(arguments.len())
            .map_err(|_| DefaultCallableBodyTypeArgumentsBuildError::TooManyArguments)?;
        Ok(Self(DefaultCallableBodyTypeArgumentsKindV1::Explicit(
            arguments,
        )))
    }

    pub const fn is_lexical(&self) -> bool {
        matches!(&self.0, DefaultCallableBodyTypeArgumentsKindV1::Lexical)
    }

    pub fn explicit_arguments(&self) -> Option<&[SignatureTypeKey]> {
        match &self.0 {
            DefaultCallableBodyTypeArgumentsKindV1::Lexical => None,
            DefaultCallableBodyTypeArgumentsKindV1::Explicit(arguments) => Some(arguments),
        }
    }
}

impl WireEncode for DefaultCallableBodyTypeArgumentsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            DefaultCallableBodyTypeArgumentsKindV1::Lexical => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            DefaultCallableBodyTypeArgumentsKindV1::Explicit(arguments) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encode_sequence(encoder, arguments)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCallableBodyTypeArgumentsV1(DecodedDefaultCallableBodyTypeArgumentsKindV1);

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedDefaultCallableBodyTypeArgumentsKindV1 {
    Lexical,
    Explicit(Vec<DecodedSignatureTypeKey>),
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
        R: crate::SignatureTypeReferenceResolver<E>,
    {
        match self.0 {
            DecodedDefaultCallableBodyTypeArgumentsKindV1::Lexical => {
                Ok(DefaultCallableBodyTypeArgumentsV1::lexical())
            }
            DecodedDefaultCallableBodyTypeArgumentsKindV1::Explicit(arguments) => {
                let count = u32::try_from(arguments.len()).map_err(|_| {
                    DefaultCallableBodyTypeArgumentsResolutionError::TooManyArguments
                })?;
                let mut resolved = Vec::with_capacity(count as usize);
                for (index, argument) in arguments.into_iter().enumerate() {
                    resolved.push(argument.resolve(resolver).map_err(|error| {
                        DefaultCallableBodyTypeArgumentsResolutionError::Argument { index, error }
                    })?);
                }
                Ok(DefaultCallableBodyTypeArgumentsV1(
                    DefaultCallableBodyTypeArgumentsKindV1::Explicit(resolved),
                ))
            }
        }
    }
}

impl WireEncode for DecodedDefaultCallableBodyTypeArgumentsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            DecodedDefaultCallableBodyTypeArgumentsKindV1::Lexical => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            DecodedDefaultCallableBodyTypeArgumentsKindV1::Explicit(arguments) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encode_sequence(encoder, arguments)
            }
        }
    }
}

impl WireDecode for DecodedDefaultCallableBodyTypeArgumentsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self(DecodedDefaultCallableBodyTypeArgumentsKindV1::Lexical))
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder.decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
                    })
                    .map(DecodedDefaultCallableBodyTypeArgumentsKindV1::Explicit)
                    .map(Self)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCaptureResolutionError<E, L> {
    Source(L),
    ValueType(E),
    FirstUseOrigin(SourceOriginResolutionError<E>),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultCaptureResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "invalid default capture source: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid default capture type: {error}"),
            Self::FirstUseOrigin(error) => {
                write!(
                    formatter,
                    "invalid default capture first-use origin: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultCaptureResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCaptureIndexError<E> {
    Source(E),
}

impl<E: fmt::Display> fmt::Display for DefaultCaptureIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => {
                write!(formatter, "cannot index default capture source: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultCaptureIndexError<E> {}

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
}

impl<E: fmt::Display> fmt::Display for DefaultCallableBodyTypeArgumentsResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyArguments => {
                formatter.write_str("default callable body type argument count exceeds u32")
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

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
