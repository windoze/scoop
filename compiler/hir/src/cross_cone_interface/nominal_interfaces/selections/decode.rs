use super::*;
use crate::{DecodedInheritanceSourceSlotSelectionV1, NominalInterfaceRecordResolver};
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedNominalDispatchSelectionV1 {
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    selection: DecodedInheritanceSourceSlotSelectionV1,
}

impl WireEncode for DecodedNominalDispatchSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(2)?;
        self.selection.encode(encoder)
    }
}

impl WireDecode for DecodedNominalDispatchSelectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            selection: decoder.field(2, DecodedInheritanceSourceSlotSelectionV1::decode)?,
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
                let slot = resolver.resolve(record.slot).map_err(Error::Reference)?;
                let selection = match record.selection {
                    Decoded::Abstract => Selection::Abstract,
                    Decoded::Concrete(target) => {
                        Selection::Concrete(target.resolve(resolver).map_err(Error::Reference)?)
                    }
                    Decoded::InterfaceDefault(target) => Selection::InterfaceDefault(
                        target.resolve(resolver).map_err(Error::Reference)?,
                    ),
                };
                Ok(NominalDispatchSelectionV1::new(slot, selection))
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let records = decoder.decode_array(|d, _| DecodedNominalDispatchSelectionV1::decode(d))?;
        let path = decoder.path().clone();
        let count = records.len() as u64;
        // Account for the complete resolution and canonical-order pass before
        // invoking an identity resolver outside the decoder.
        decoder.meter().charge_collection_slots(count, &path)?;
        decoder
            .meter()
            .charge_nodes(count.saturating_mul(2), &path)?;
        decoder
            .meter()
            .charge_edges(count.saturating_mul(2), &path)?;
        decoder
            .meter()
            .charge_work(count.saturating_mul(128), &path)?;
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
