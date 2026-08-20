use std::fmt;

use scoop_identity::{DecodedPersistentId, PersistentExactTypeId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::wire;

mod source_shapes;
mod table;
#[cfg(test)]
mod tests;
mod validation;

pub use source_shapes::*;
pub use table::*;
pub use validation::*;

/// A semantic fact produced in HIR, independent of target byte size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZstStatus {
    ZeroSized,
    NonZero,
}

impl WireEncode for ZstStatus {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::tag(
            encoder,
            1,
            match self {
                Self::ZeroSized => 1,
                Self::NonZero => 2,
            },
        )
    }
}

impl WireDecode for ZstStatus {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::ZeroSized),
            2 => Ok(Self::NonZero),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactTypeGcV1 {
    GcFree,
    ContainsManagedReferences,
}

impl ExactTypeGcV1 {
    pub const fn is_gc_free(self) -> bool {
        matches!(self, Self::GcFree)
    }
}

impl WireEncode for ExactTypeGcV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::tag(
            encoder,
            1,
            match self {
                Self::GcFree => 1,
                Self::ContainsManagedReferences => 2,
            },
        )
    }
}

impl WireDecode for ExactTypeGcV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::GcFree),
            2 => Ok(Self::ContainsManagedReferences),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactTypeKindV1 {
    Value { zst: ZstStatus },
    Reference,
}

impl WireEncode for ExactTypeKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Value { zst } => {
                wire::tag(encoder, 2, 1)?;
                encoder.field(1)?;
                zst.encode(encoder)
            }
            Self::Reference => wire::tag(encoder, 1, 2),
        }
    }
}

impl WireDecode for ExactTypeKindV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 2)?;
                Ok(Self::Value {
                    zst: decoder.field(1, ZstStatus::decode)?,
                })
            }
            2 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::Reference)
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactTypeFactsV1 {
    exact: PersistentExactTypeId,
    kind: ExactTypeKindV1,
    gc: ExactTypeGcV1,
}

impl ExactTypeFactsV1 {
    pub fn try_new(
        exact: PersistentExactTypeId,
        kind: ExactTypeKindV1,
        gc: ExactTypeGcV1,
    ) -> Result<Self, ExactTypeFactsBuildError> {
        match (kind, gc) {
            (ExactTypeKindV1::Reference, ExactTypeGcV1::GcFree) => {
                Err(ExactTypeFactsBuildError::GcFreeReference)
            }
            (
                ExactTypeKindV1::Value {
                    zst: ZstStatus::ZeroSized,
                },
                ExactTypeGcV1::ContainsManagedReferences,
            ) => Err(ExactTypeFactsBuildError::ManagedZeroSizedValue),
            _ => Ok(Self { exact, kind, gc }),
        }
    }

    pub const fn exact(self) -> PersistentExactTypeId {
        self.exact
    }
    pub const fn kind(self) -> ExactTypeKindV1 {
        self.kind
    }
    pub const fn gc(self) -> ExactTypeGcV1 {
        self.gc
    }
}

impl WireEncode for ExactTypeFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_facts(encoder, &self.exact, self.kind, self.gc)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedExactTypeFactsV1 {
    exact: DecodedPersistentId<PersistentExactTypeId>,
    kind: ExactTypeKindV1,
    gc: ExactTypeGcV1,
}

impl DecodedExactTypeFactsV1 {
    pub fn resolve<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
    ) -> Result<ExactTypeFactsV1, ExactTypeFactsResolutionError<R::Error>> {
        let exact = resolver
            .resolve(self.exact)
            .map_err(ExactTypeFactsResolutionError::Exact)?;
        ExactTypeFactsV1::try_new(exact, self.kind, self.gc)
            .map_err(ExactTypeFactsResolutionError::Facts)
    }
}

impl WireEncode for DecodedExactTypeFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_facts(encoder, &self.exact, self.kind, self.gc)
    }
}

impl WireDecode for DecodedExactTypeFactsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            exact: decoder.field(1, DecodedPersistentId::decode)?,
            kind: decoder.field(2, ExactTypeKindV1::decode)?,
            gc: decoder.field(3, ExactTypeGcV1::decode)?,
        })
    }
}

fn encode_facts(
    encoder: &mut Encoder,
    exact: &impl WireEncode,
    kind: ExactTypeKindV1,
    gc: ExactTypeGcV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    exact.encode(encoder)?;
    encoder.field(2)?;
    kind.encode(encoder)?;
    encoder.field(3)?;
    gc.encode(encoder)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactTypeFactsBuildError {
    GcFreeReference,
    ManagedZeroSizedValue,
}

impl fmt::Display for ExactTypeFactsBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::GcFreeReference => "a reference value must contain a managed reference",
            Self::ManagedZeroSizedValue => "a zero-sized value must be GC-free",
        })
    }
}
impl std::error::Error for ExactTypeFactsBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExactTypeFactsResolutionError<E> {
    Exact(E),
    Facts(ExactTypeFactsBuildError),
}

impl<E: fmt::Display> fmt::Display for ExactTypeFactsResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(error) => write!(f, "invalid type-facts exact identity: {error}"),
            Self::Facts(error) => write!(f, "invalid type facts: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ExactTypeFactsResolutionError<E> {}
