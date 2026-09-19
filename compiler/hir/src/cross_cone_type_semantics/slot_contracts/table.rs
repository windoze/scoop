use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalInheritanceSlotContractsV1 {
    records: Vec<InheritanceSlotContractV1>,
}
impl CanonicalInheritanceSlotContractsV1 {
    pub fn try_new(
        mut records: Vec<InheritanceSlotContractV1>,
    ) -> Result<Self, InheritanceSlotContractBuildError> {
        records.sort_unstable_by_key(InheritanceSlotContractV1::slot);
        for (index, pair) in records.windows(2).enumerate() {
            if pair[0].slot() == pair[1].slot() {
                return Err(InheritanceSlotContractBuildError::SlotOrder { index: index + 1 });
            }
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[InheritanceSlotContractV1] {
        &self.records
    }
    pub fn get(&self, slot: PersistentDispatchSlotId) -> Option<&InheritanceSlotContractV1> {
        self.records
            .binary_search_by_key(&slot, InheritanceSlotContractV1::slot)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalInheritanceSlotContractsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceSlotContractsV1 {
    records: Vec<DecodedInheritanceSlotContractV1>,
}
impl DecodedCanonicalInheritanceSlotContractsV1 {
    pub fn resolve<R: InheritanceSlotResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalInheritanceSlotContractsV1, InheritanceSlotResolutionError<E>> {
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), &WirePath::root())
            .map_err(InheritanceSlotResolutionError::Resource)?;
        for decoded in self.records {
            let record = decoded.resolve(resolver, meter)?;
            if records
                .last()
                .is_some_and(|previous: &InheritanceSlotContractV1| {
                    previous.slot() >= record.slot()
                })
            {
                return Err(InheritanceSlotResolutionError::Contract(
                    InheritanceSlotContractBuildError::SlotOrder {
                        index: records.len(),
                    },
                ));
            }
            records.push(record);
        }
        Ok(CanonicalInheritanceSlotContractsV1 { records })
    }
}
impl WireEncode for DecodedCanonicalInheritanceSlotContractsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalInheritanceSlotContractsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedInheritanceSlotContractV1::decode(decoder))
            .map(|records| Self { records })
    }
}
