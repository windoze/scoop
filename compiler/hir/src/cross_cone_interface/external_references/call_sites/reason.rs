//! A call's source route or the language operation that requires it.

use scoop_identity::{DecodedPersistentId, PersistentExactTypeId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDependencyCallReasonV1 {
    SourceBinding(Vec<u32>),
    SourceDeclaration,
    CastFailure { checked_type: PersistentExactTypeId },
}

impl WireEncode for HirDependencyCallReasonV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (fields, tag) = match self {
            Self::SourceBinding(_) => (2, 1),
            Self::CastFailure { .. } => (2, 2),
            Self::SourceDeclaration => (1, 3),
        };
        encoder.map(fields)?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        match self {
            Self::SourceBinding(indices) => {
                encoder.field(1)?;
                super::encode_indices(indices, encoder)
            }
            Self::CastFailure { checked_type } => {
                encoder.field(1)?;
                checked_type.encode(encoder)
            }
            Self::SourceDeclaration => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedHirDependencyCallReasonV1 {
    SourceBinding(Vec<u32>),
    SourceDeclaration,
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
            Self::SourceDeclaration => HirDependencyCallReasonV1::SourceDeclaration,
            Self::CastFailure { checked_type } => HirDependencyCallReasonV1::CastFailure {
                checked_type: resolver.resolve(checked_type)?,
            },
        })
    }
}

impl WireEncode for DecodedHirDependencyCallReasonV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (fields, tag) = match self {
            Self::SourceBinding(_) => (2, 1),
            Self::CastFailure { .. } => (2, 2),
            Self::SourceDeclaration => (1, 3),
        };
        encoder.map(fields)?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        match self {
            Self::SourceBinding(indices) => {
                encoder.field(1)?;
                super::encode_indices(indices, encoder)
            }
            Self::CastFailure { checked_type } => {
                encoder.field(1)?;
                checked_type.encode(encoder)
            }
            Self::SourceDeclaration => Ok(()),
        }
    }
}

impl WireDecode for DecodedHirDependencyCallReasonV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 | 2 => 2,
            3 => 1,
            tag => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if fields != expected {
            return Err(wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
            ));
        }
        match tag {
            1 => decoder
                .field(1, |d| d.decode_array(|d, _| d.u32()))
                .map(Self::SourceBinding),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|checked_type| Self::CastFailure { checked_type }),
            3 => Ok(Self::SourceDeclaration),
            _ => unreachable!("call reason tag was checked above"),
        }
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
