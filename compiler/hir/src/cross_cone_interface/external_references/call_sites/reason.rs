//! A call's source route or the language operation that requires it.

use scoop_identity::{DecodedPersistentId, PersistentExactTypeId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDependencyCallReasonV1 {
    SourceBinding(Vec<u32>),
    CastFailure { checked_type: PersistentExactTypeId },
}

impl WireEncode for HirDependencyCallReasonV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::SourceBinding(_) => 1,
            Self::CastFailure { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::SourceBinding(indices) => super::encode_indices(indices, encoder),
            Self::CastFailure { checked_type } => checked_type.encode(encoder),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedHirDependencyCallReasonV1 {
    SourceBinding(Vec<u32>),
    CastFailure {
        checked_type: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedHirDependencyCallReasonV1 {
    pub(super) fn resolve<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
    ) -> Result<HirDependencyCallReasonV1, R::Error> {
        Ok(match self {
            Self::SourceBinding(indices) => HirDependencyCallReasonV1::SourceBinding(indices),
            Self::CastFailure { checked_type } => HirDependencyCallReasonV1::CastFailure {
                checked_type: resolver.resolve(checked_type)?,
            },
        })
    }
}

impl WireEncode for DecodedHirDependencyCallReasonV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::SourceBinding(_) => 1,
            Self::CastFailure { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::SourceBinding(indices) => super::encode_indices(indices, encoder),
            Self::CastFailure { checked_type } => checked_type.encode(encoder),
        }
    }
}

impl WireDecode for DecodedHirDependencyCallReasonV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, |d| d.decode_array(|d, _| d.u32()))
                .map(Self::SourceBinding),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|checked_type| Self::CastFailure { checked_type }),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
