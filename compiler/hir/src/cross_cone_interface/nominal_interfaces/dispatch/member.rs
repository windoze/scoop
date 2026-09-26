//! Complete interface member identity and override references.

use std::fmt;

use scoop_identity::{DecodedPersistentId, PersistentDispatchSlotId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{CanonicalPersistentIdsV1, DecodedCanonicalPersistentIdsV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceSourceMemberV1 {
    slot: PersistentDispatchSlotId,
    overrides: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
}
impl InterfaceSourceMemberV1 {
    pub const fn new(
        slot: PersistentDispatchSlotId,
        overrides: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
    ) -> Self {
        Self { slot, overrides }
    }
    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }
    pub const fn overrides(&self) -> &CanonicalPersistentIdsV1<PersistentDispatchSlotId> {
        &self.overrides
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInterfaceSourceMemberV1 {
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    overrides: DecodedCanonicalPersistentIdsV1<PersistentDispatchSlotId>,
}
macro_rules! encode_member {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.slot.encode(encoder)?;
                encoder.field(2)?;
                self.overrides.encode(encoder)
            }
        }
    };
}
encode_member!(InterfaceSourceMemberV1);
encode_member!(DecodedInterfaceSourceMemberV1);
impl WireDecode for DecodedInterfaceSourceMemberV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            overrides: decoder.field(2, DecodedCanonicalPersistentIdsV1::decode)?,
        })
    }
}
impl DecodedInterfaceSourceMemberV1 {
    pub(crate) fn resolve_member<
        R: PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
        E,
    >(
        self,
        resolver: &mut R,
    ) -> Result<InterfaceSourceMemberV1, InterfaceSourceMemberResolutionError<E>> {
        Ok(InterfaceSourceMemberV1::new(
            resolver
                .resolve(self.slot)
                .map_err(InterfaceSourceMemberResolutionError::Reference)?,
            self.overrides
                .resolve(resolver)
                .map_err(InterfaceSourceMemberResolutionError::Overrides)?,
        ))
    }
}

#[derive(Debug)]
pub enum InterfaceSourceMemberResolutionError<E> {
    Reference(E),
    Overrides(crate::CanonicalPersistentIdSetValidationError<PersistentDispatchSlotId, E>),
}
impl<E: fmt::Display> fmt::Display for InterfaceSourceMemberResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(e) => e.fmt(f),
            Self::Overrides(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InterfaceSourceMemberResolutionError<E> {}
