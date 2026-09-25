//! Interface declaration order and typed override edges, independent of schemas.

use super::*;
use crate::CanonicalPersistentIdsV1;
use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId};
use scoop_wire::{Encoder, WireEncode};
use std::collections::BTreeSet;

mod wire;
pub use wire::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceSourceMemberV1 {
    slot: PersistentDispatchSlotId,
    overrides: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
}
impl InterfaceSourceMemberV1 {
    pub const fn new(
        slot: PersistentDispatchSlotId,
        overrides: CanonicalPersistentIdsV1<PersistentDispatchSlotId>,
    ) -> Self {
        Self { slot, overrides }
    }
    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }
    pub const fn overrides(&self) -> &CanonicalPersistentIdsV1<PersistentDispatchSlotId> {
        &self.overrides
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceSourceDispatchV1 {
    owner: PersistentExactTypeId,
    parents: Vec<PersistentExactTypeId>,
    members: Vec<InterfaceSourceMemberV1>,
}
impl InterfaceSourceDispatchV1 {
    pub fn try_new(
        owner: PersistentExactTypeId,
        parents: Vec<PersistentExactTypeId>,
        members: Vec<InterfaceSourceMemberV1>,
    ) -> Result<Self, SourceInventoryError> {
        let invalid = |reason| SourceInventoryError::InvalidInterfaceDispatch { owner, reason };
        if !unique(parents.iter().copied()) {
            return Err(invalid("duplicate direct parent"));
        }
        if !unique(members.iter().map(InterfaceSourceMemberV1::slot)) {
            return Err(invalid("duplicate directly declared member"));
        }
        for member in &members {
            if member.overrides.values().contains(&member.slot) {
                return Err(invalid("a member overrides itself"));
            }
        }
        Ok(Self {
            owner,
            parents,
            members,
        })
    }
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub fn parents(&self) -> &[PersistentExactTypeId] {
        &self.parents
    }
    pub fn members(&self) -> &[InterfaceSourceMemberV1] {
        &self.members
    }
}

fn unique<T: Ord>(items: impl Iterator<Item = T>) -> bool {
    let mut seen = BTreeSet::new();
    items.into_iter().all(|item| seen.insert(item))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInterfaceSourceDispatchesV1 {
    records: Vec<InterfaceSourceDispatchV1>,
}
impl CanonicalInterfaceSourceDispatchesV1 {
    pub fn try_new(
        mut records: Vec<InterfaceSourceDispatchV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(InterfaceSourceDispatchV1::owner);
        Self::from_ordered(records)
    }
    fn from_ordered(records: Vec<InterfaceSourceDispatchV1>) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            InterfaceSourceDispatchV1::owner,
            "interface dispatch declarations",
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[InterfaceSourceDispatchV1] {
        &self.records
    }
    pub fn get(&self, owner: PersistentExactTypeId) -> Option<&InterfaceSourceDispatchV1> {
        self.records
            .binary_search_by_key(&owner, InterfaceSourceDispatchV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalInterfaceSourceDispatchesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}
