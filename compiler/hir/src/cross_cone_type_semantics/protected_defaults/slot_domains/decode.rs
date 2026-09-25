use std::fmt;

use scoop_identity::{DecodedPersistentId, PersistentDispatchSlotId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::{
    CanonicalProtectedDefaultSlotCallDomainsV1, ProtectedDefaultSlotCallDomainV1,
    ProtectedDefaultSlotCallDomainsBuildError, wire,
};
use crate::{
    DecodedPersistentAccessDomainV1, PersistentAccessResolutionError, PersistentAccessResolver,
    PersistentSlotContractDomainV1,
};

pub trait ProtectedDefaultSlotCallDomainResolver<E>:
    PersistentAccessResolver<E> + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
{
}
impl<R, E> ProtectedDefaultSlotCallDomainResolver<E> for R where
    R: PersistentAccessResolver<E> + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedDefaultSlotCallDomainV1 {
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    domain: DecodedPersistentAccessDomainV1,
}

impl DecodedProtectedDefaultSlotCallDomainV1 {
    pub fn resolve<R: ProtectedDefaultSlotCallDomainResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedDefaultSlotCallDomainV1, ProtectedDefaultSlotCallDomainResolutionError<E>>
    {
        let slot = resolver
            .resolve(self.slot)
            .map_err(ProtectedDefaultSlotCallDomainResolutionError::Identity)?;
        let domain = self
            .domain
            .resolve(resolver)
            .map_err(ProtectedDefaultSlotCallDomainResolutionError::Domain)?;
        Ok(ProtectedDefaultSlotCallDomainV1::new(
            slot,
            PersistentSlotContractDomainV1::new(domain),
        ))
    }
}

impl WireEncode for DecodedProtectedDefaultSlotCallDomainV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(2)?;
        self.domain.encode(encoder)
    }
}

impl WireDecode for DecodedProtectedDefaultSlotCallDomainV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            domain: decoder.field(2, DecodedPersistentAccessDomainV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalProtectedDefaultSlotCallDomainsV1 {
    records: Vec<DecodedProtectedDefaultSlotCallDomainV1>,
}

impl DecodedCanonicalProtectedDefaultSlotCallDomainsV1 {
    pub fn resolve<R: ProtectedDefaultSlotCallDomainResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<
        CanonicalProtectedDefaultSlotCallDomainsV1,
        ProtectedDefaultSlotCallDomainResolutionError<E>,
    > {
        let mut records = Vec::new();
        let path = WirePath::root();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &path)
            .map_err(ProtectedDefaultSlotCallDomainResolutionError::Resource)?;
        for decoded in self.records {
            let record = decoded.resolve(resolver)?;

            if let Some(previous) = records.last() {
                super::validate_pair(previous, &record, records.len())
                    .map_err(ProtectedDefaultSlotCallDomainResolutionError::Build)?;
            }
            records.push(record);
        }
        u32::try_from(records.len()).map_err(|_| {
            ProtectedDefaultSlotCallDomainResolutionError::Build(
                ProtectedDefaultSlotCallDomainsBuildError::TooMany,
            )
        })?;
        Ok(CanonicalProtectedDefaultSlotCallDomainsV1 { records })
    }
}

impl WireEncode for DecodedCanonicalProtectedDefaultSlotCallDomainsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

impl WireDecode for DecodedCanonicalProtectedDefaultSlotCallDomainsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedProtectedDefaultSlotCallDomainV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Debug)]
pub enum ProtectedDefaultSlotCallDomainResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Domain(PersistentAccessResolutionError<E>),
    Build(ProtectedDefaultSlotCallDomainsBuildError),
}

impl<E: fmt::Display> fmt::Display for ProtectedDefaultSlotCallDomainResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => write!(f, "invalid protected default slot identity: {error}"),
            Self::Domain(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultSlotCallDomainResolutionError<E>
{
}
