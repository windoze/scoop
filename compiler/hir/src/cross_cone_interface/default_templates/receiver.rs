use std::fmt;

use scoop_identity::{DecodedSignatureTypeKey, LocalValueSelector, SignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{TemplateLocalIndexResolver, TemplateLocalSelectorResolver};
use crate::SignatureTypeReferenceResolver;

mod semantics;

pub use semantics::TemplateReceiverSemanticValidationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateReceiverV1 {
    local: LocalValueSelector,
    value_type: SignatureTypeKey,
}

impl TemplateReceiverV1 {
    pub fn try_new(
        local: LocalValueSelector,
        value_type: SignatureTypeKey,
    ) -> Result<Self, TemplateReceiverBuildError> {
        if local != LocalValueSelector::This {
            return Err(TemplateReceiverBuildError::ExpectedThis(local));
        }
        Ok(Self { local, value_type })
    }

    pub const fn local(&self) -> &LocalValueSelector {
        &self.local
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    /// Builds a wire-only projection whose semantic local selector is
    /// replaced by its canonical table index.
    pub fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedTemplateReceiverV1<'_>, TemplateReceiverIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let local_index = resolver
            .resolve_template_local_index(&self.local)
            .map_err(TemplateReceiverIndexError::Local)?;
        Ok(IndexedTemplateReceiverV1 {
            receiver: self,
            local_index,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTemplateReceiverV1 {
    local_index: u32,
    value_type: DecodedSignatureTypeKey,
}

impl DecodedTemplateReceiverV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<TemplateReceiverV1, TemplateReceiverResolutionError<E, L::Error>>
    where
        R: SignatureTypeReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let local = locals
            .resolve_template_local_selector(self.local_index)
            .map_err(TemplateReceiverResolutionError::Local)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(TemplateReceiverResolutionError::ValueType)?;
        TemplateReceiverV1::try_new(local, value_type)
            .map_err(TemplateReceiverResolutionError::Record)
    }
}

impl WireEncode for DecodedTemplateReceiverV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.local_index))?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

impl WireDecode for DecodedTemplateReceiverV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            local_index: decoder.field(1, Decoder::u32)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct IndexedTemplateReceiverV1<'a> {
    receiver: &'a TemplateReceiverV1,
    local_index: u32,
}

impl WireEncode for IndexedTemplateReceiverV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.local_index))?;
        encoder.field(2)?;
        self.receiver.value_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionalTemplateReceiverV1 {
    Absent,
    Present(TemplateReceiverV1),
}

impl OptionalTemplateReceiverV1 {
    pub const fn receiver(&self) -> Option<&TemplateReceiverV1> {
        match self {
            Self::Absent => None,
            Self::Present(receiver) => Some(receiver),
        }
    }

    /// Builds a wire-only projection whose semantic local selector is
    /// replaced by its canonical table index.
    pub fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedOptionalTemplateReceiverV1<'_>, TemplateReceiverIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self {
            Self::Absent => Ok(IndexedOptionalTemplateReceiverV1::Absent),
            Self::Present(receiver) => receiver
                .index_local(resolver)
                .map(IndexedOptionalTemplateReceiverV1::Present),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedOptionalTemplateReceiverV1 {
    Absent,
    Present(DecodedTemplateReceiverV1),
}

impl DecodedOptionalTemplateReceiverV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<OptionalTemplateReceiverV1, TemplateReceiverResolutionError<E, L::Error>>
    where
        R: SignatureTypeReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Absent => Ok(OptionalTemplateReceiverV1::Absent),
            Self::Present(receiver) => receiver
                .resolve(resolver, locals)
                .map(OptionalTemplateReceiverV1::Present),
        }
    }
}

impl WireEncode for DecodedOptionalTemplateReceiverV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty_sum(encoder, 1),
            Self::Present(receiver) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                receiver.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedOptionalTemplateReceiverV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Absent)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedTemplateReceiverV1::decode)
                    .map(Self::Present)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug)]
pub enum IndexedOptionalTemplateReceiverV1<'a> {
    Absent,
    Present(IndexedTemplateReceiverV1<'a>),
}

impl WireEncode for IndexedOptionalTemplateReceiverV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty_sum(encoder, 1),
            Self::Present(receiver) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                receiver.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateReceiverBuildError {
    ExpectedThis(LocalValueSelector),
}

impl fmt::Display for TemplateReceiverBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedThis(actual) => write!(
                formatter,
                "default-template receiver must reference This, found {actual:?}"
            ),
        }
    }
}

impl std::error::Error for TemplateReceiverBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateReceiverResolutionError<E, L> {
    Local(L),
    ValueType(E),
    Record(TemplateReceiverBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for TemplateReceiverResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(
                formatter,
                "invalid default-template receiver local: {error}"
            ),
            Self::ValueType(error) => {
                write!(formatter, "invalid default-template receiver type: {error}")
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for TemplateReceiverResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateReceiverIndexError<E> {
    Local(E),
}

impl<E: fmt::Display> fmt::Display for TemplateReceiverIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => {
                write!(formatter, "cannot index default-template receiver: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TemplateReceiverIndexError<E> {}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
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
