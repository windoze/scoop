use super::*;
use crate::cross_cone_type_semantics::wire;
use crate::{
    CallableDeclarationIdResolver, DecodedDefaultSourceAccessDomainV1,
    DefaultSourceAccessResolutionError as Error, PersistentAccessResolver,
};
use scoop_identity::DecodedCallableTemplateOrigin;
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
};

pub trait DefaultSourceAccessWitnessResolver<E>:
    PersistentAccessResolver<E> + CallableDeclarationIdResolver<E>
{
}
impl<R, E> DefaultSourceAccessWitnessResolver<E> for R where
    R: PersistentAccessResolver<E> + CallableDeclarationIdResolver<E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedOptionalDefaultSourceSlotDomainV1 {
    Absent,
    Present(DecodedDefaultSourceAccessDomainV1),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultSourceAccessWitnessV1 {
    owner: DecodedCallableTemplateOrigin,
    direct: DecodedDefaultSourceAccessDomainV1,
    slot: DecodedOptionalDefaultSourceSlotDomainV1,
    target: DecodedDefaultSourceAccessDomainV1,
}
impl DecodedDefaultSourceAccessWitnessV1 {
    pub fn resolve<R: DefaultSourceAccessWitnessResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceAccessWitnessV1, Error<E>> {
        let path = WirePath::root();
        meter
            .check_semantic_depth(4, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let owner = self.owner.resolve(resolver).map_err(Error::Owner)?;
        let direct = self.direct.resolve(resolver, meter)?;
        let slot = match self.slot {
            DecodedOptionalDefaultSourceSlotDomainV1::Absent => {
                OptionalDefaultSourceSlotDomainV1::Absent
            }
            DecodedOptionalDefaultSourceSlotDomainV1::Present(domain) => {
                OptionalDefaultSourceSlotDomainV1::Present(domain.resolve(resolver, meter)?)
            }
        };
        let target = self.target.resolve(resolver, meter)?;
        DefaultSourceAccessWitnessV1::try_new(owner, direct, slot, target).map_err(Error::Build)
    }
}
impl WireDecode for DecodedDefaultSourceAccessWitnessV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.expect_map(4)?;
        Ok(Self {
            owner: d.field(1, DecodedCallableTemplateOrigin::decode)?,
            direct: d.field(2, DecodedDefaultSourceAccessDomainV1::decode)?,
            slot: d.field(3, DecodedOptionalDefaultSourceSlotDomainV1::decode)?,
            target: d.field(4, DecodedDefaultSourceAccessDomainV1::decode)?,
        })
    }
}
impl WireDecode for DecodedOptionalDefaultSourceSlotDomainV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = d.map()?;
        match d.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(d, fields, 1)?;
                Ok(Self::Absent)
            }
            2 => {
                wire::expect_fields(d, fields, 2)?;
                d.field(1, DecodedDefaultSourceAccessDomainV1::decode)
                    .map(Self::Present)
            }
            tag => Err(wire::error(d, WireErrorKind::UnknownTag { tag })),
        }
    }
}
macro_rules! encode_slot {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::Absent => wire::tag(e, 1, 1),
                    Self::Present(domain) => {
                        wire::tag(e, 2, 2)?;
                        e.field(1)?;
                        domain.encode(e)
                    }
                }
            }
        }
    };
}
macro_rules! encode_witness {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(4)?;
                e.field(1)?;
                self.owner.encode(e)?;
                e.field(2)?;
                self.direct.encode(e)?;
                e.field(3)?;
                self.slot.encode(e)?;
                e.field(4)?;
                self.target.encode(e)
            }
        }
    };
}
encode_slot!(OptionalDefaultSourceSlotDomainV1);
encode_slot!(DecodedOptionalDefaultSourceSlotDomainV1);
encode_witness!(DefaultSourceAccessWitnessV1);
encode_witness!(DecodedDefaultSourceAccessWitnessV1);
