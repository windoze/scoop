use std::fmt;

use scoop_identity::{
    DecodedSignatureTypeKey, LocalValueSelector, SignatureTypeKey, SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedExportDefinitionSourceV1, ExportDefinitionSourceV1, TemplateLocalIndexResolver,
    TemplateLocalSelectorResolver,
};

mod binding;
mod body_arguments;
pub use body_arguments::*;
mod source;
use binding::DecodedDefaultCaptureBindingV1;
pub use binding::DefaultCaptureBindingV1;
pub use source::DefaultCaptureSourceV1;
use source::IndexedCaptureSource;

/// One hidden closure-environment input in provider ABI order.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCaptureV1 {
    source: DefaultCaptureSourceV1,
    value_type: SignatureTypeKey,
    first_use_origin: ExportDefinitionSourceV1,
    binding: DefaultCaptureBindingV1,
}

impl DefaultCaptureV1 {
    pub const fn new(
        source: LocalValueSelector,
        value_type: SignatureTypeKey,
        first_use_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            source: DefaultCaptureSourceV1::Local(source),
            value_type,
            first_use_origin,
            binding: DefaultCaptureBindingV1::Source,
        }
    }

    pub const fn from_enclosing_capture(
        index: u32,
        value_type: SignatureTypeKey,
        first_use_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            source: DefaultCaptureSourceV1::EnclosingCapture(index),
            value_type,
            first_use_origin,
            binding: DefaultCaptureBindingV1::Source,
        }
    }

    pub const fn source(&self) -> &DefaultCaptureSourceV1 {
        &self.source
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn first_use_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.first_use_origin
    }

    pub const fn binding(&self) -> &DefaultCaptureBindingV1 {
        &self.binding
    }

    pub fn with_binding(mut self, binding: DefaultCaptureBindingV1) -> Self {
        self.binding = binding;
        self
    }

    pub fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultCaptureV1<'_>, DefaultCaptureIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let source_index = match &self.source {
            DefaultCaptureSourceV1::Local(selector) => IndexedCaptureSource::Local(
                resolver
                    .resolve_template_local_index(selector)
                    .map_err(DefaultCaptureIndexError::Source)?,
            ),
            DefaultCaptureSourceV1::EnclosingCapture(index) => {
                IndexedCaptureSource::EnclosingCapture(*index)
            }
        };
        Ok(IndexedDefaultCaptureV1 {
            capture: self,
            source_index,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCaptureV1 {
    source_index: IndexedCaptureSource,
    value_type: DecodedSignatureTypeKey,
    first_use_origin: DecodedExportDefinitionSourceV1,
    binding: DecodedDefaultCaptureBindingV1,
}

impl DecodedDefaultCaptureV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultCaptureV1, DefaultCaptureResolutionError<E, L::Error>>
    where
        R: crate::DefaultNestedCallableReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        Ok(DefaultCaptureV1 {
            source: match self.source_index {
                IndexedCaptureSource::Local(index) => DefaultCaptureSourceV1::Local(
                    locals
                        .resolve_template_local_selector(index)
                        .map_err(DefaultCaptureResolutionError::Source)?,
                ),
                IndexedCaptureSource::EnclosingCapture(index) => {
                    DefaultCaptureSourceV1::EnclosingCapture(index)
                }
            },
            value_type: self
                .value_type
                .resolve(resolver)
                .map_err(DefaultCaptureResolutionError::ValueType)?,
            first_use_origin: self
                .first_use_origin
                .resolve(resolver)
                .map_err(DefaultCaptureResolutionError::FirstUseOrigin)?,
            binding: self.binding.resolve(resolver)?,
        })
    }
}

impl WireEncode for DecodedDefaultCaptureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.source_index.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.first_use_origin.encode(encoder)?;
        encoder.field(4)?;
        self.binding.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultCaptureV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            source_index: decoder.field(1, IndexedCaptureSource::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            first_use_origin: decoder.field(3, DecodedExportDefinitionSourceV1::decode)?,
            binding: decoder.field(4, DecodedDefaultCaptureBindingV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct IndexedDefaultCaptureV1<'a> {
    capture: &'a DefaultCaptureV1,
    source_index: IndexedCaptureSource,
}

impl WireEncode for IndexedDefaultCaptureV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.source_index.encode(encoder)?;
        encoder.field(2)?;
        self.capture.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.capture.first_use_origin.encode(encoder)?;
        encoder.field(4)?;
        self.capture.binding.encode(encoder)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCaptureResolutionError<E, L> {
    Source(L),
    ValueType(E),
    FirstUseOrigin(SourceOriginResolutionError<E>),
    BindingOwner(E),
    BindingOrigin(SourceOriginResolutionError<E>),
    BindingShape(crate::TemplateLocalRecordBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultCaptureResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "invalid default capture source: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid default capture type: {error}"),
            Self::BindingOwner(error) => {
                write!(formatter, "invalid capture binding owner: {error}")
            }
            Self::BindingOrigin(error) => {
                write!(formatter, "invalid capture binding origin: {error}")
            }
            Self::BindingShape(error) => error.fmt(formatter),
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
