use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedDefinitionOrigin, DefinitionOrigin, PersistentIdResolver,
    PersistentKeyResolver, PersistentSourceContextId, SourceContextKey,
    SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

/// A definition-side source location embedded in the cross-Cone interface.
///
/// This remains a distinct semantic type even though its wire is deliberately
/// byte-identical to the foundation `DefinitionOrigin` schema.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDefinitionSourceV1 {
    origin: DefinitionOrigin,
}

impl ExportDefinitionSourceV1 {
    pub const fn new(origin: DefinitionOrigin) -> Self {
        Self { origin }
    }

    pub const fn origin(&self) -> &DefinitionOrigin {
        &self.origin
    }
}

impl WireEncode for ExportDefinitionSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportDefinitionSourceV1 {
    origin: DecodedDefinitionOrigin,
}

impl DecodedExportDefinitionSourceV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportDefinitionSourceV1, SourceOriginResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        self.origin
            .resolve(resolver)
            .map(ExportDefinitionSourceV1::new)
    }
}

impl WireEncode for DecodedExportDefinitionSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.origin.encode(encoder)
    }
}

impl WireDecode for DecodedExportDefinitionSourceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        DecodedDefinitionOrigin::decode(decoder).map(|origin| Self { origin })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExportDefinitionSourcesV1 {
    sources: Vec<ExportDefinitionSourceV1>,
}

impl CanonicalExportDefinitionSourcesV1 {
    pub fn try_new(
        mut sources: Vec<ExportDefinitionSourceV1>,
    ) -> Result<Self, ExportDefinitionSourceSetBuildError> {
        sources.sort_unstable();
        if let Some(source) = sources
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0].clone())
        {
            return Err(ExportDefinitionSourceSetBuildError::Duplicate(source));
        }
        Ok(Self { sources })
    }

    pub fn sources(&self) -> &[ExportDefinitionSourceV1] {
        &self.sources
    }

    pub fn contains(&self, source: &ExportDefinitionSourceV1) -> bool {
        self.sources.binary_search(source).is_ok()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

impl WireEncode for CanonicalExportDefinitionSourcesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.sources.len() as u64)?;
        for source in &self.sources {
            source.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExportDefinitionSourcesV1 {
    sources: Vec<DecodedExportDefinitionSourceV1>,
}

impl DecodedCanonicalExportDefinitionSourcesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExportDefinitionSourcesV1, ExportDefinitionSourceSetValidationError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        let mut sources = Vec::<ExportDefinitionSourceV1>::with_capacity(self.sources.len());
        for (index, source) in self.sources.into_iter().enumerate() {
            let source = source.resolve(resolver).map_err(|error| {
                ExportDefinitionSourceSetValidationError::Source { index, error }
            })?;
            if let Some(previous) = sources.last() {
                match previous.cmp(&source) {
                    std::cmp::Ordering::Equal => {
                        return Err(ExportDefinitionSourceSetValidationError::Duplicate { index });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(
                            ExportDefinitionSourceSetValidationError::NonCanonicalOrder { index },
                        );
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            sources.push(source);
        }
        Ok(CanonicalExportDefinitionSourcesV1 { sources })
    }
}

impl WireEncode for DecodedCanonicalExportDefinitionSourcesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.sources.len() as u64)?;
        for source in &self.sources {
            source.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalExportDefinitionSourcesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExportDefinitionSourceV1::decode(decoder))
            .map(|sources| Self { sources })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportDefinitionSourceSetBuildError {
    Duplicate(ExportDefinitionSourceV1),
}

impl fmt::Display for ExportDefinitionSourceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(source) => {
                write!(formatter, "duplicate export definition source {source:?}")
            }
        }
    }
}

impl std::error::Error for ExportDefinitionSourceSetBuildError {}

#[derive(Debug)]
pub enum ExportDefinitionSourceSetValidationError<E> {
    Source {
        index: usize,
        error: SourceOriginResolutionError<E>,
    },
    Duplicate {
        index: usize,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for ExportDefinitionSourceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { index, error } => {
                write!(
                    formatter,
                    "invalid export definition source at index {index}: {error}"
                )
            }
            Self::Duplicate { index } => {
                write!(
                    formatter,
                    "duplicate export definition source at index {index}"
                )
            }
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical export definition source order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefinitionSourceSetValidationError<E>
{
}

#[cfg(test)]
mod tests;
