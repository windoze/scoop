use scoop_identity::{
    DecodedPersistentId, PersistentFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::wire;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InheritanceCallableDeclarationV1 {
    Function(PersistentFunctionId),
    Getter(PersistentPropertyAccessorId),
    Setter(PersistentPropertyAccessorId),
}

impl WireEncode for InheritanceCallableDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
            Self::Function(id) => (1, id),
            Self::Getter(id) => (2, id),
            Self::Setter(id) => (3, id),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        id.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedInheritanceCallableDeclarationV1 {
    Function(DecodedPersistentId<PersistentFunctionId>),
    Getter(DecodedPersistentId<PersistentPropertyAccessorId>),
    Setter(DecodedPersistentId<PersistentPropertyAccessorId>),
}
impl DecodedInheritanceCallableDeclarationV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<InheritanceCallableDeclarationV1, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        match self {
            Self::Function(id) => resolver
                .resolve(id)
                .map(InheritanceCallableDeclarationV1::Function),
            Self::Getter(id) => resolver
                .resolve(id)
                .map(InheritanceCallableDeclarationV1::Getter),
            Self::Setter(id) => resolver
                .resolve(id)
                .map(InheritanceCallableDeclarationV1::Setter),
        }
    }
}
impl WireEncode for DecodedInheritanceCallableDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
            Self::Function(id) => (1, id),
            Self::Getter(id) => (2, id),
            Self::Setter(id) => (3, id),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        id.encode(encoder)
    }
}
impl WireDecode for DecodedInheritanceCallableDeclarationV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Getter),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Setter),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
