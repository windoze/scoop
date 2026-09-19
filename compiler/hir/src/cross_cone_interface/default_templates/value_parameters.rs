mod resolution_nodes;

use std::fmt;

use scoop_identity::LocalValueSelector;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{TemplateLocalIndexResolver, TemplateLocalSelectorResolver};

mod semantics;

pub use semantics::TemplateValueParameterSemanticValidationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateValueParameterV1 {
    position: u32,
    local: LocalValueSelector,
}

impl TemplateValueParameterV1 {
    pub fn try_new(
        position: u32,
        local: LocalValueSelector,
    ) -> Result<Self, TemplateValueParameterBuildError> {
        let expected = LocalValueSelector::Parameter {
            declaration_index: position,
        };
        if local != expected {
            return Err(TemplateValueParameterBuildError::LocalMismatch {
                position,
                actual: local,
            });
        }
        Ok(Self { position, local })
    }

    pub const fn position(&self) -> u32 {
        self.position
    }

    pub const fn local(&self) -> &LocalValueSelector {
        &self.local
    }

    fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedTemplateValueParameterV1<'_>, TemplateValueParameterIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let local_index = resolver
            .resolve_template_local_index(&self.local)
            .map_err(TemplateValueParameterIndexError::Local)?;
        Ok(IndexedTemplateValueParameterV1 {
            parameter: self,
            local_index,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTemplateValueParameterV1 {
    position: u32,
    local_index: u32,
}

impl DecodedTemplateValueParameterV1 {
    pub fn resolve<L>(
        self,
        locals: &mut L,
    ) -> Result<TemplateValueParameterV1, TemplateValueParameterResolutionError<L::Error>>
    where
        L: TemplateLocalSelectorResolver,
    {
        let local = locals
            .resolve_template_local_selector(self.local_index)
            .map_err(TemplateValueParameterResolutionError::Local)?;
        TemplateValueParameterV1::try_new(self.position, local)
            .map_err(TemplateValueParameterResolutionError::Record)
    }
}

impl WireEncode for DecodedTemplateValueParameterV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.position))?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.local_index))
    }
}

impl WireDecode for DecodedTemplateValueParameterV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            position: decoder.field(1, Decoder::u32)?,
            local_index: decoder.field(2, Decoder::u32)?,
        })
    }
}

#[derive(Debug)]
pub struct IndexedTemplateValueParameterV1<'a> {
    parameter: &'a TemplateValueParameterV1,
    local_index: u32,
}

impl WireEncode for IndexedTemplateValueParameterV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.parameter.position))?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.local_index))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTemplateValueParametersV1 {
    parameters: Vec<TemplateValueParameterV1>,
    len: u32,
}

impl CanonicalTemplateValueParametersV1 {
    pub fn try_new(
        mut parameters: Vec<TemplateValueParameterV1>,
    ) -> Result<Self, TemplateValueParameterListBuildError> {
        let len = u32::try_from(parameters.len())
            .map_err(|_| TemplateValueParameterListBuildError::TooMany)?;
        parameters.sort_unstable_by_key(TemplateValueParameterV1::position);
        validate_positions(&parameters)?;
        Ok(Self { parameters, len })
    }

    pub fn parameters(&self) -> &[TemplateValueParameterV1] {
        &self.parameters
    }

    pub const fn len_u32(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    pub fn get(&self, position: u32) -> Option<&TemplateValueParameterV1> {
        usize::try_from(position)
            .ok()
            .and_then(|position| self.parameters.get(position))
    }

    /// Builds a wire-only projection whose semantic local selectors are
    /// replaced by canonical table indices.
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<
        IndexedCanonicalTemplateValueParametersV1<'_>,
        TemplateValueParameterListIndexError<I::Error>,
    >
    where
        I: TemplateLocalIndexResolver,
    {
        let mut parameters = Vec::with_capacity(self.parameters.len());
        for (index, parameter) in self.parameters.iter().enumerate() {
            parameters.push(parameter.index_local(resolver).map_err(|error| {
                TemplateValueParameterListIndexError::Parameter { index, error }
            })?);
        }
        Ok(IndexedCanonicalTemplateValueParametersV1 { parameters })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalTemplateValueParametersV1 {
    parameters: Vec<DecodedTemplateValueParameterV1>,
}

impl DecodedCanonicalTemplateValueParametersV1 {
    pub fn resolve<L>(
        self,
        locals: &mut L,
    ) -> Result<
        CanonicalTemplateValueParametersV1,
        TemplateValueParameterListValidationError<L::Error>,
    >
    where
        L: TemplateLocalSelectorResolver,
    {
        let len = u32::try_from(self.parameters.len())
            .map_err(|_| TemplateValueParameterListValidationError::TooMany)?;
        let mut parameters = Vec::with_capacity(self.parameters.len());
        for (index, parameter) in self.parameters.into_iter().enumerate() {
            let expected = u32::try_from(index)
                .map_err(|_| TemplateValueParameterListValidationError::TooMany)?;
            if parameter.position != expected {
                return Err(
                    TemplateValueParameterListValidationError::UnexpectedPosition {
                        index,
                        expected,
                        actual: parameter.position,
                    },
                );
            }
            parameters.push(parameter.resolve(locals).map_err(|error| {
                TemplateValueParameterListValidationError::Parameter { index, error }
            })?);
        }
        Ok(CanonicalTemplateValueParametersV1 { parameters, len })
    }
}

impl WireEncode for DecodedCanonicalTemplateValueParametersV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalTemplateValueParametersV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedTemplateValueParameterV1::decode(decoder))
            .map(|parameters| Self { parameters })
    }
}

#[derive(Debug)]
pub struct IndexedCanonicalTemplateValueParametersV1<'a> {
    parameters: Vec<IndexedTemplateValueParameterV1<'a>>,
}

impl WireEncode for IndexedCanonicalTemplateValueParametersV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateValueParameterBuildError {
    LocalMismatch {
        position: u32,
        actual: LocalValueSelector,
    },
}

impl fmt::Display for TemplateValueParameterBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalMismatch { position, actual } => write!(
                formatter,
                "default-template value parameter {position} references {actual:?}"
            ),
        }
    }
}

impl std::error::Error for TemplateValueParameterBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateValueParameterResolutionError<E> {
    Local(E),
    Record(TemplateValueParameterBuildError),
}

impl<E: fmt::Display> fmt::Display for TemplateValueParameterResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => {
                write!(
                    formatter,
                    "invalid default-template value-parameter local: {error}"
                )
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TemplateValueParameterResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateValueParameterIndexError<E> {
    Local(E),
}

impl<E: fmt::Display> fmt::Display for TemplateValueParameterIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(
                formatter,
                "cannot index default-template value-parameter local: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TemplateValueParameterIndexError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateValueParameterListBuildError {
    TooMany,
    UnexpectedPosition {
        index: usize,
        expected: u32,
        actual: u32,
    },
}

impl fmt::Display for TemplateValueParameterListBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => {
                formatter.write_str("default-template value-parameter count exceeds u32")
            }
            Self::UnexpectedPosition {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "default-template value parameter at index {index} has position {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for TemplateValueParameterListBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateValueParameterListValidationError<E> {
    TooMany,
    UnexpectedPosition {
        index: usize,
        expected: u32,
        actual: u32,
    },
    Parameter {
        index: usize,
        error: TemplateValueParameterResolutionError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for TemplateValueParameterListValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => {
                formatter.write_str("default-template value-parameter count exceeds u32")
            }
            Self::UnexpectedPosition {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "default-template value parameter at index {index} has position {actual}, expected {expected}"
            ),
            Self::Parameter { index, error } => write!(
                formatter,
                "invalid default-template value parameter at index {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TemplateValueParameterListValidationError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateValueParameterListIndexError<E> {
    Parameter {
        index: usize,
        error: TemplateValueParameterIndexError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for TemplateValueParameterListIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameter { index, error } => write!(
                formatter,
                "cannot index default-template value parameter at index {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TemplateValueParameterListIndexError<E> {}

fn validate_positions(
    parameters: &[TemplateValueParameterV1],
) -> Result<(), TemplateValueParameterListBuildError> {
    for (expected, (index, parameter)) in (0_u32..).zip(parameters.iter().enumerate()) {
        if parameter.position != expected {
            return Err(TemplateValueParameterListBuildError::UnexpectedPosition {
                index,
                expected,
                actual: parameter.position,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
