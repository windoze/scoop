use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedSourceIdentity, PersistentExactTypeId,
    PersistentIdResolver, SourceIdentityResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;
use crate::{DecodedSourceNominalId, SourceNominalIdResolver};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedPersistentAccessConstraintV1 {
    Cone(DecodedPersistentId<ConeIdentity>),
    File(DecodedSourceIdentity),
    LexicalOwner(DecodedSourceNominalId),
    SubclassesOf(DecodedPersistentId<PersistentExactTypeId>),
}

pub trait PersistentAccessResolver<E>:
    SourceNominalIdResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
{
}
impl<R, E> PersistentAccessResolver<E> for R where
    R: SourceNominalIdResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
{
}

impl DecodedPersistentAccessConstraintV1 {
    pub fn resolve<R: PersistentAccessResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<PersistentAccessConstraintV1, PersistentAccessResolutionError<E>> {
        match self {
            Self::Cone(id) => resolver
                .resolve(id)
                .map(PersistentAccessConstraintV1::Cone)
                .map_err(PersistentAccessResolutionError::Identity),
            Self::File(source) => source
                .resolve(resolver)
                .map(PersistentAccessConstraintV1::File)
                .map_err(PersistentAccessResolutionError::Source),
            Self::LexicalOwner(owner) => owner
                .resolve(resolver)
                .map(PersistentAccessConstraintV1::LexicalOwner)
                .map_err(PersistentAccessResolutionError::Identity),
            Self::SubclassesOf(id) => resolver
                .resolve(id)
                .map(PersistentAccessConstraintV1::SubclassesOf)
                .map_err(PersistentAccessResolutionError::Identity),
        }
    }
}

impl WireEncode for DecodedPersistentAccessConstraintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, value): (u64, &dyn WireEncode) = match self {
            Self::Cone(value) => (1, value),
            Self::File(value) => (2, value),
            Self::LexicalOwner(value) => (3, value),
            Self::SubclassesOf(value) => (4, value),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        value.encode(encoder)
    }
}

impl WireDecode for DecodedPersistentAccessConstraintV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Cone),
            2 => decoder
                .field(1, DecodedSourceIdentity::decode)
                .map(Self::File),
            3 => decoder
                .field(1, DecodedSourceNominalId::decode)
                .map(Self::LexicalOwner),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::SubclassesOf),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedPersistentAccessDomainV1 {
    Empty,
    Conjunction(Vec<DecodedPersistentAccessConstraintV1>),
}

impl DecodedPersistentAccessDomainV1 {
    pub fn resolve_metered<R: PersistentAccessResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<PersistentAccessDomainV1, PersistentAccessResolutionError<E>> {
        let path = scoop_wire::WirePath::root();
        meter
            .charge_nodes(1, &path)
            .map_err(PersistentAccessResolutionError::Resource)?;
        if let Self::Conjunction(constraints) = &self {
            meter
                .charge_collection_slots(constraints.len() as u64, &path)
                .map_err(PersistentAccessResolutionError::Resource)?;
            meter
                .charge_work(constraints.len() as u64, &path)
                .map_err(PersistentAccessResolutionError::Resource)?;
        }
        self.resolve(resolver)
    }

    pub fn resolve<R: PersistentAccessResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<PersistentAccessDomainV1, PersistentAccessResolutionError<E>> {
        match self {
            Self::Empty => Ok(PersistentAccessDomainV1::empty()),
            Self::Conjunction(constraints) => {
                let constraints = constraints
                    .into_iter()
                    .map(|constraint| constraint.resolve(resolver))
                    .collect::<Result<Vec<_>, _>>()?;
                PersistentAccessDomainV1::from_ordered(constraints)
                    .map_err(PersistentAccessResolutionError::Domain)
            }
        }
    }
}

impl WireEncode for DecodedPersistentAccessDomainV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Empty => wire::tag(encoder, 1, 1),
            Self::Conjunction(constraints) => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                wire::sequence(encoder, constraints)
            }
        }
    }
}

impl WireDecode for DecodedPersistentAccessDomainV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::Empty)
            }
            2 => {
                wire::expect_fields(decoder, fields, 2)?;
                decoder
                    .field(1, |decoder| {
                        decoder.decode_array(|decoder, _| {
                            DecodedPersistentAccessConstraintV1::decode(decoder)
                        })
                    })
                    .map(Self::Conjunction)
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalAccessDomainsV1 {
    lookup: DecodedPersistentAccessDomainV1,
    inheritance: DecodedPersistentAccessDomainV1,
    slot: DecodedPersistentAccessDomainV1,
}

impl DecodedNominalAccessDomainsV1 {
    pub fn resolve_metered<R: PersistentAccessResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<NominalAccessDomainsV1, PersistentAccessResolutionError<E>> {
        meter
            .charge_nodes(1, &scoop_wire::WirePath::root())
            .map_err(PersistentAccessResolutionError::Resource)?;
        Ok(NominalAccessDomainsV1::new(
            PersistentLookupDomainV1::new(self.lookup.resolve_metered(resolver, meter)?),
            PersistentInheritanceDomainV1::new(self.inheritance.resolve_metered(resolver, meter)?),
            PersistentSlotContractDomainV1::new(self.slot.resolve_metered(resolver, meter)?),
        ))
    }
    pub fn resolve<R: PersistentAccessResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalAccessDomainsV1, PersistentAccessResolutionError<E>> {
        Ok(NominalAccessDomainsV1::new(
            PersistentLookupDomainV1::new(self.lookup.resolve(resolver)?),
            PersistentInheritanceDomainV1::new(self.inheritance.resolve(resolver)?),
            PersistentSlotContractDomainV1::new(self.slot.resolve(resolver)?),
        ))
    }
}

impl WireEncode for DecodedNominalAccessDomainsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.lookup.encode(encoder)?;
        encoder.field(2)?;
        self.inheritance.encode(encoder)?;
        encoder.field(3)?;
        self.slot.encode(encoder)
    }
}

impl WireDecode for DecodedNominalAccessDomainsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            lookup: decoder.field(1, DecodedPersistentAccessDomainV1::decode)?,
            inheritance: decoder.field(2, DecodedPersistentAccessDomainV1::decode)?,
            slot: decoder.field(3, DecodedPersistentAccessDomainV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub enum PersistentAccessResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Source(SourceIdentityResolutionError<E>),
    Domain(PersistentAccessDomainError),
}
impl<E: fmt::Display> fmt::Display for PersistentAccessResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => write!(f, "invalid persistent access identity: {error}"),
            Self::Source(error) => write!(f, "invalid persistent access source: {error}"),
            Self::Domain(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for PersistentAccessResolutionError<E> {}
