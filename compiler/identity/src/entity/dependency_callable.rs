//! Refined declaration identity for executable ordinary dependency calls.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedPersistentId, PersistentFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
    StrongCallableDefinitionOwner,
};

/// The only source declaration kinds that M23-5 may refine into an ordinary
/// dependency callable target.
///
/// Constructors and generated callables intentionally have no variant here;
/// generic and native eligibility is checked by the HIR/MIR bridge authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DependencyCallableDeclarationId {
    Function(PersistentFunctionId),
    PropertyAccessor(PersistentPropertyAccessorId),
}

impl DependencyCallableDeclarationId {
    pub const fn implementation(self) -> StrongCallableDefinitionOwner {
        match self {
            Self::Function(id) => StrongCallableDefinitionOwner::Function(id),
            Self::PropertyAccessor(id) => StrongCallableDefinitionOwner::PropertyAccessor(id),
        }
    }
}

impl WireEncode for DependencyCallableDeclarationId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_sum(encoder, 1, id),
            Self::PropertyAccessor(id) => encode_sum(encoder, 2, id),
        }
    }
}

/// Untrusted wire form of [`DependencyCallableDeclarationId`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedDependencyCallableDeclarationId {
    Function(DecodedPersistentId<PersistentFunctionId>),
    PropertyAccessor(DecodedPersistentId<PersistentPropertyAccessorId>),
}

impl DecodedDependencyCallableDeclarationId {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DependencyCallableDeclarationId, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        match self {
            Self::Function(id) => resolver
                .resolve(id)
                .map(DependencyCallableDeclarationId::Function),
            Self::PropertyAccessor(id) => resolver
                .resolve(id)
                .map(DependencyCallableDeclarationId::PropertyAccessor),
        }
    }
}

impl WireEncode for DecodedDependencyCallableDeclarationId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_sum(encoder, 1, id),
            Self::PropertyAccessor(id) => encode_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedDependencyCallableDeclarationId {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        if fields != 2 {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::PropertyAccessor),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

fn encode_sum<T: WireEncode>(
    encoder: &mut Encoder,
    tag: u64,
    value: &T,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests;
