//! Implementation choices used while assembling inheritance slot contracts.

use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId};

use super::*;
use crate::{InheritanceSlotSchemaRoleV1, InheritanceSourceSlotSelectionV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InheritanceSourceSlotSelectionRecordV1 {
    owner: PersistentExactTypeId,
    role: InheritanceSlotSchemaRoleV1,
    receiver: PersistentExactTypeId,
    slot: PersistentDispatchSlotId,
    selection: InheritanceSourceSlotSelectionV1,
}

impl InheritanceSourceSlotSelectionRecordV1 {
    pub const fn new(
        owner: PersistentExactTypeId,
        role: InheritanceSlotSchemaRoleV1,
        receiver: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        selection: InheritanceSourceSlotSelectionV1,
    ) -> Self {
        Self {
            owner,
            role,
            receiver,
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
    pub const fn receiver(self) -> PersistentExactTypeId {
        self.receiver
    }
    pub const fn selection(self) -> InheritanceSourceSlotSelectionV1 {
        self.selection
    }

    fn key(
        &self,
    ) -> (
        PersistentExactTypeId,
        InheritanceSlotSchemaRoleV1,
        PersistentDispatchSlotId,
    ) {
        (self.owner, self.role, self.slot)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSourceSlotSelectionsV1 {
    records: Vec<InheritanceSourceSlotSelectionRecordV1>,
}

impl CanonicalInheritanceSourceSlotSelectionsV1 {
    pub fn try_new(
        mut records: Vec<InheritanceSourceSlotSelectionRecordV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(InheritanceSourceSlotSelectionRecordV1::key);
        Self::from_ordered(records)
    }

    fn from_ordered(
        records: Vec<InheritanceSourceSlotSelectionRecordV1>,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            InheritanceSourceSlotSelectionRecordV1::key,
            "inheritance slot selections",
        )?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[InheritanceSourceSlotSelectionRecordV1] {
        &self.records
    }

    pub fn get(
        &self,
        owner: PersistentExactTypeId,
        role: InheritanceSlotSchemaRoleV1,
        slot: PersistentDispatchSlotId,
    ) -> Option<InheritanceSourceSlotSelectionRecordV1> {
        self.records
            .binary_search_by_key(
                &(owner, role, slot),
                InheritanceSourceSlotSelectionRecordV1::key,
            )
            .ok()
            .map(|index| self.records[index])
    }
}
