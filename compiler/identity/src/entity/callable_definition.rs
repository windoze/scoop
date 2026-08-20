//! Definition targets shared by concrete callable bindings and dispatch tables.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CallableBodyKey, CallableBodyResolutionError, CallableOdrMemberId,
    DecodedStrongCallableDefinitionOwner, StrongCallableDefinitionOwner,
};
use crate::{
    DecodedPersistentId, OdrMemberId, OdrMemberKey, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentIdResolver, PersistentKeyResolver,
    PersistentPropertyAccessorId,
};

/// A real callable definition; entry gateways have separate roles.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDefinitionOwner {
    Strong(StrongCallableDefinitionOwner),
    Odr(CallableOdrMemberId),
}

impl CallableDefinitionOwner {
    pub const fn strong_owner(self) -> Option<StrongCallableDefinitionOwner> {
        match self {
            Self::Strong(owner) => Some(owner),
            Self::Odr(_) => None,
        }
    }

    pub const fn body_key(self) -> CallableBodyKey {
        match self {
            Self::Strong(owner) => CallableBodyKey::strong(owner),
            Self::Odr(member) => CallableBodyKey::odr(member),
        }
    }
}

impl From<StrongCallableDefinitionOwner> for CallableDefinitionOwner {
    fn from(owner: StrongCallableDefinitionOwner) -> Self {
        Self::Strong(owner)
    }
}

impl scoop_wire::RuntimeEncode for CallableDefinitionOwner {
    fn runtime_encode(
        &self,
        encoder: &mut scoop_wire::RuntimeEncoder,
    ) -> Result<(), scoop_wire::RuntimeEncodeError> {
        self.body_key().runtime_encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCallableDefinitionOwner {
    Strong(DecodedStrongCallableDefinitionOwner),
    Odr(DecodedPersistentId<OdrMemberId>),
}

impl DecodedCallableDefinitionOwner {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableDefinitionOwner, CallableBodyResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentKeyResolver<OdrMemberId, OdrMemberKey, Error = E>,
    {
        match self {
            Self::Strong(owner) => owner
                .resolve(resolver)
                .map(CallableDefinitionOwner::Strong)
                .map_err(CallableBodyResolutionError::Reference),
            Self::Odr(member) => {
                let key = resolver
                    .resolve_key(member)
                    .map_err(CallableBodyResolutionError::Reference)?;
                CallableOdrMemberId::from_key(&key)
                    .map(CallableDefinitionOwner::Odr)
                    .map_err(CallableBodyResolutionError::Member)
            }
        }
    }
}

impl WireEncode for CallableDefinitionOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Strong(owner) => encode(encoder, 1, owner),
            Self::Odr(member) => encode(encoder, 2, &member.member()),
        }
    }
}

impl WireEncode for DecodedCallableDefinitionOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Strong(owner) => encode(encoder, 1, owner),
            Self::Odr(member) => encode(encoder, 2, member),
        }
    }
}

impl WireDecode for DecodedCallableDefinitionOwner {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedStrongCallableDefinitionOwner::decode)
                .map(Self::Strong),
            2 => decoder.field(1, DecodedPersistentId::decode).map(Self::Odr),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

fn encode(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}
