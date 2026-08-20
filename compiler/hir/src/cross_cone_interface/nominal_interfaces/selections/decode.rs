use super::role::DecodedNominalDispatchSelectionRoleV1;
use super::*;
use crate::{DecodedInheritanceSourceSlotSelectionV1, NominalInterfaceRecordResolver};
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedNominalDispatchSelectionV1 {
    role: DecodedNominalDispatchSelectionRoleV1,
    receiver: scoop_identity::DecodedSignatureTypeKey,
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    selection: DecodedInheritanceSourceSlotSelectionV1,
}

impl WireEncode for DecodedNominalDispatchSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(0)?;
        self.role.encode(encoder)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(2)?;
        self.selection.encode(encoder)?;
        encoder.field(3)?;
        self.receiver.encode(encoder)
    }
}

impl WireDecode for DecodedNominalDispatchSelectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            role: decoder.field(0, DecodedNominalDispatchSelectionRoleV1::decode)?,
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            selection: decoder.field(2, DecodedInheritanceSourceSlotSelectionV1::decode)?,
            receiver: decoder.field(3, scoop_identity::DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalDispatchSelectionsV1 {
    records: Vec<DecodedNominalDispatchSelectionV1>,
}

impl DecodedCanonicalNominalDispatchSelectionsV1 {
    pub fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalNominalDispatchSelectionsV1, NominalDispatchSelectionResolutionError<E>>
    {
        use DecodedInheritanceSourceSlotSelectionV1 as Decoded;
        use InheritanceSourceSlotSelectionV1 as Selection;
        use NominalDispatchSelectionResolutionError as Error;
        let records = self
            .records
            .into_iter()
            .map(|record| {
                let role = record.role.resolve(resolver).map_err(Error::Reference)?;
                let receiver = record
                    .receiver
                    .resolve(resolver)
                    .map_err(Error::Reference)?;
                let slot = resolver.resolve(record.slot).map_err(Error::Reference)?;
                let selection = match record.selection {
                    Decoded::Abstract(target) => {
                        Selection::Abstract(target.resolve(resolver).map_err(Error::Reference)?)
                    }
                    Decoded::Concrete(target) => {
                        Selection::Concrete(target.resolve(resolver).map_err(Error::Reference)?)
                    }
                    Decoded::InterfaceDefault(target) => Selection::InterfaceDefault(
                        target.resolve(resolver).map_err(Error::Reference)?,
                    ),
                };
                Ok(NominalDispatchSelectionV1::new(
                    role, receiver, slot, selection,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        CanonicalNominalDispatchSelectionsV1::from_ordered(records).map_err(Error::Order)
    }
}

impl WireEncode for DecodedCanonicalNominalDispatchSelectionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalNominalDispatchSelectionsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let records = decoder.decode_array(|d, _| DecodedNominalDispatchSelectionV1::decode(d))?;

        // Account for the complete resolution and canonical-order pass before
        // invoking an identity resolver outside the decoder.

        Ok(Self { records })
    }
}

#[derive(Debug)]
pub enum NominalDispatchSelectionResolutionError<E> {
    Reference(E),
    Order(NominalDispatchSelectionError),
}
impl<E: std::fmt::Display> std::fmt::Display for NominalDispatchSelectionResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(f),
            Self::Order(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for NominalDispatchSelectionResolutionError<E>
{
}
