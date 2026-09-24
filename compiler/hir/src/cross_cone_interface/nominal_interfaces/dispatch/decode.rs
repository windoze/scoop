use super::*;
use crate::{
    DecodedInterfaceSourceMemberV1, InterfaceSourceMemberResolutionError,
    NominalInterfaceRecordResolver,
};
use scoop_identity::{DecodedPersistentId, DecodedSignatureTypeKey};
use scoop_wire::{Decoder, WireDecode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNominalDispatchOrderV1 {
    NonVirtual,
    Class {
        slots: Vec<DecodedPersistentId<PersistentDispatchSlotId>>,
    },
    Interface {
        parents: Vec<DecodedSignatureTypeKey>,
        members: Vec<DecodedInterfaceSourceMemberV1>,
    },
}

impl DecodedNominalDispatchOrderV1 {
    pub fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalDispatchOrderV1, NominalDispatchOrderResolutionError<E>> {
        use NominalDispatchOrderResolutionError as Error;
        let value = match self {
            Self::NonVirtual => NominalDispatchOrderV1::NonVirtual,
            Self::Class { slots } => NominalDispatchOrderV1::Class {
                slots: slots
                    .into_iter()
                    .map(|slot| resolver.resolve(slot))
                    .collect::<Result<_, _>>()
                    .map_err(Error::Reference)?,
            },
            Self::Interface { parents, members } => NominalDispatchOrderV1::Interface {
                parents: parents
                    .into_iter()
                    .map(|parent| parent.resolve(resolver))
                    .collect::<Result<_, _>>()
                    .map_err(Error::Reference)?,
                members: members
                    .into_iter()
                    .map(|member| member.resolve_member(resolver))
                    .collect::<Result<_, _>>()
                    .map_err(Error::Member)?,
            },
        };
        value.validate_structure().map_err(Error::Order)?;
        Ok(value)
    }
}

impl WireDecode for DecodedNominalDispatchOrderV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let start = decoder.position();
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 2,
            3 => 3,
            tag => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        if fields != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        let value = match tag {
            1 => Self::NonVirtual,
            2 => Self::Class {
                slots: decoder
                    .field(1, |d| d.decode_array(|d, _| DecodedPersistentId::decode(d)))?,
            },
            _ => Self::Interface {
                parents: decoder.field(1, |d| {
                    d.decode_array(|d, _| DecodedSignatureTypeKey::decode(d))
                })?,
                members: decoder.field(2, |d| {
                    d.decode_array(|d, _| DecodedInterfaceSourceMemberV1::decode(d))
                })?,
            },
        };
        let count = match &value {
            Self::NonVirtual => 0,
            Self::Class { slots } => slots.len(),
            Self::Interface { parents, members } => {
                let path = decoder.path().clone().field(2);
                for (index, member) in members.iter().enumerate() {
                    member.charge_member_resolution(
                        decoder.meter(),
                        &path.clone().index(index as u64),
                    )?;
                }
                parents.len() + members.len()
            }
        } as u64;
        let path = decoder.path().clone();
        let bytes = decoder.position().saturating_sub(start);
        decoder
            .meter()
            .charge_collection_slots(count.saturating_mul(3), &path)?;
        decoder.meter().charge_owned_bytes(bytes, &path)?;
        decoder.meter().charge_work(
            bytes.saturating_mul(1 + u64::from(count.max(1).ilog2())),
            &path,
        )?;
        Ok(value)
    }
}

impl WireEncode for DecodedNominalDispatchOrderV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NonVirtual => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Class { slots } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                sequence(encoder, slots)
            }
            Self::Interface { parents, members } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                sequence(encoder, parents)?;
                encoder.field(2)?;
                sequence(encoder, members)
            }
        }
    }
}

#[derive(Debug)]
pub enum NominalDispatchOrderResolutionError<E> {
    Order(NominalDispatchOrderError),
    Reference(E),
    Member(InterfaceSourceMemberResolutionError<E>),
}
impl<E: std::fmt::Display> std::fmt::Display for NominalDispatchOrderResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Order(e) => e.fmt(f),
            Self::Reference(e) => e.fmt(f),
            Self::Member(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for NominalDispatchOrderResolutionError<E> {}
