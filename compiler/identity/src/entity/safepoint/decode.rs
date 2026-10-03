use std::num::NonZeroU64;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{RuntimeTypeId, SafepointId, SafepointSiteKey, SafepointSiteRole};
use crate::{DecodedPersistentId, PersistentCallableBodyId, PersistentIdResolver};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedSafepointSiteKey {
    owner: DecodedPersistentId<PersistentCallableBodyId>,
    role: SafepointSiteRole,
    ordinal: u32,
}

impl DecodedSafepointSiteKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<SafepointSiteKey, E>
    where
        R: PersistentIdResolver<PersistentCallableBodyId, Error = E>,
    {
        let owner = resolver.resolve(self.owner)?;
        Ok(SafepointSiteKey::new(owner, self.role, self.ordinal))
    }
}

impl WireEncode for DecodedSafepointSiteKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.ordinal))
    }
}

impl WireDecode for DecodedSafepointSiteKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            role: decoder.field(2, SafepointSiteRole::decode)?,
            ordinal: decoder.field(3, Decoder::u32)?,
        })
    }
}

impl WireDecode for SafepointSiteRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ManagedPoll),
            2 => Ok(Self::ManagedCall),
            3 => Ok(Self::ManagedInvoke),
            4 => Ok(Self::NativeSafeTransition),
            5 => Ok(Self::NativeBorrowedTransition),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

macro_rules! impl_derived_id_decoding {
    ($name:ident) => {
        impl WireDecode for $name {
            fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
                NonZeroU64::new(decoder.unsigned()?)
                    .map(Self)
                    .ok_or_else(|| {
                        WireError::new(
                            WireErrorKind::IntegerOutOfRange,
                            decoder.path().clone(),
                            Some(decoder.position()),
                        )
                    })
            }
        }
    };
}

impl_derived_id_decoding!(RuntimeTypeId);
impl_derived_id_decoding!(SafepointId);

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

#[cfg(test)]
mod tests;
