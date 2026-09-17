use std::fmt;

use scoop_identity::{
    DecodedPersistentId, LocalValueSelector, PersistentIdResolver, PersistentPropertyId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{TemplateLocalIndexResolver, TemplateLocalSelectorResolver};

/// Addressable storage accepted by a portable default body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultPlaceV1 {
    Local { local: LocalValueSelector },
    Global { property: PersistentPropertyId },
}

impl DefaultPlaceV1 {
    /// Replaces a semantic local selector with its canonical local-table
    /// index. The semantic type itself deliberately has no `WireEncode`
    /// implementation, so a process-local selector cannot bypass indexing.
    pub fn index_local<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultPlaceV1, DefaultPlaceIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self {
            Self::Local { local } => resolver
                .resolve_template_local_index(local)
                .map(|local_index| IndexedDefaultPlaceV1::Local { local_index })
                .map_err(DefaultPlaceIndexError::Local),
            Self::Global { property } => Ok(IndexedDefaultPlaceV1::Global {
                property: *property,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexedDefaultPlaceV1 {
    Local { local_index: u32 },
    Global { property: PersistentPropertyId },
}

impl WireEncode for IndexedDefaultPlaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Local { .. } => 1,
            Self::Global { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Local { local_index } => encoder.unsigned(u64::from(*local_index)),
            Self::Global { property } => property.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedDefaultPlaceV1 {
    Local {
        local_index: u32,
    },
    Global {
        property: DecodedPersistentId<PersistentPropertyId>,
    },
}

impl DecodedDefaultPlaceV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultPlaceV1, DefaultPlaceResolutionError<E, L::Error>>
    where
        R: PersistentIdResolver<PersistentPropertyId, Error = E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Local { local_index } => locals
                .resolve_template_local_selector(local_index)
                .map(|local| DefaultPlaceV1::Local { local })
                .map_err(DefaultPlaceResolutionError::Local),
            Self::Global { property } => resolver
                .resolve(property)
                .map(|property| DefaultPlaceV1::Global { property })
                .map_err(DefaultPlaceResolutionError::Global),
        }
    }
}

impl WireEncode for DecodedDefaultPlaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Local { .. } => 1,
            Self::Global { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Local { local_index } => encoder.unsigned(u64::from(*local_index)),
            Self::Global { property } => property.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultPlaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, Decoder::u32)
                .map(|local_index| Self::Local { local_index }),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|property| Self::Global { property }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultPlaceResolutionError<E, L> {
    Local(L),
    Global(E),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultPlaceResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "invalid default place local: {error}"),
            Self::Global(error) => write!(formatter, "invalid default place global: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultPlaceResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultPlaceIndexError<E> {
    Local(E),
}

impl<E: fmt::Display> fmt::Display for DefaultPlaceIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "cannot index default place local: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultPlaceIndexError<E> {}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
