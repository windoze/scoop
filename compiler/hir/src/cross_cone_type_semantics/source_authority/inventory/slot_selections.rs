//! Independent source implementation decisions, indexed by exact owner and slot.

use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, Encoder, WireEncode};

use super::*;
use crate::InheritanceSourceSlotSelectionV1;

mod wire;
pub use wire::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InheritanceSourceSlotSelectionRecordV1 {
    owner: PersistentExactTypeId,
    slot: PersistentDispatchSlotId,
    selection: InheritanceSourceSlotSelectionV1,
}

impl InheritanceSourceSlotSelectionRecordV1 {
    pub const fn new(
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        selection: InheritanceSourceSlotSelectionV1,
    ) -> Self {
        Self {
            owner,
            slot,
            selection,
        }
    }

    pub const fn owner(self) -> PersistentExactTypeId {
        self.owner
    }
    pub const fn slot(self) -> PersistentDispatchSlotId {
        self.slot
    }
    pub const fn selection(self) -> InheritanceSourceSlotSelectionV1 {
        self.selection
    }

    fn key(&self) -> (PersistentExactTypeId, PersistentDispatchSlotId) {
        (self.owner, self.slot)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSourceSlotSelectionsV1 {
    records: Vec<InheritanceSourceSlotSelectionRecordV1>,
}

impl CanonicalInheritanceSourceSlotSelectionsV1 {
    pub fn try_new(
        mut records: Vec<InheritanceSourceSlotSelectionRecordV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(InheritanceSourceSlotSelectionRecordV1::key);
        Self::from_ordered(records, meter)
    }

    fn from_ordered(
        records: Vec<InheritanceSourceSlotSelectionRecordV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            InheritanceSourceSlotSelectionRecordV1::key,
            "inheritance slot selections",
            meter,
        )?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[InheritanceSourceSlotSelectionRecordV1] {
        &self.records
    }

    pub fn get(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    ) -> Option<InheritanceSourceSlotSelectionV1> {
        self.records
            .binary_search_by_key(&(owner, slot), InheritanceSourceSlotSelectionRecordV1::key)
            .ok()
            .map(|index| self.records[index].selection)
    }
}

impl WireEncode for CanonicalInheritanceSourceSlotSelectionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
