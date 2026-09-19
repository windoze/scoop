use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId};
use scoop_wire::{Encoder, WireEncode};

use super::wire;

mod decode;
mod validation;

pub use decode::*;
pub use validation::*;

/// Ordering matches canonical role bytes: the leaf ClassVtable tag precedes
/// the Interface payload tag, whose ids have a fixed byte length.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InheritanceSlotSchemaRoleV1 {
    ClassVtable,
    Interface {
        interface_exact: PersistentExactTypeId,
    },
}
impl WireEncode for InheritanceSlotSchemaRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ClassVtable => wire::tag(encoder, 1, 1),
            Self::Interface { interface_exact } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                interface_exact.encode(encoder)
            }
        }
    }
}

/// A declaration-order dispatch sequence. Slot ids are deliberately not sorted
/// here; one slot can occupy a different position in another interface schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritanceSlotSchemaV1 {
    role: InheritanceSlotSchemaRoleV1,
    slots: Vec<PersistentDispatchSlotId>,
}
impl InheritanceSlotSchemaV1 {
    pub fn try_new(
        role: InheritanceSlotSchemaRoleV1,
        slots: Vec<PersistentDispatchSlotId>,
    ) -> Result<Self, InheritanceSlotSchemaBuildError> {
        let mut seen = BTreeSet::new();
        for (position, slot) in slots.iter().enumerate() {
            if !seen.insert(*slot) {
                return Err(InheritanceSlotSchemaBuildError::DuplicateSlot {
                    position,
                    slot: *slot,
                });
            }
        }
        Ok(Self { role, slots })
    }
    pub const fn role(&self) -> InheritanceSlotSchemaRoleV1 {
        self.role
    }
    pub fn slots(&self) -> &[PersistentDispatchSlotId] {
        &self.slots
    }
}
impl WireEncode for InheritanceSlotSchemaV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.role.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.slots)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalInheritanceSlotSchemasV1 {
    records: Vec<InheritanceSlotSchemaV1>,
}
impl CanonicalInheritanceSlotSchemasV1 {
    pub fn try_new(
        mut records: Vec<InheritanceSlotSchemaV1>,
    ) -> Result<Self, InheritanceSlotSchemaBuildError> {
        records.sort_unstable_by_key(InheritanceSlotSchemaV1::role);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<InheritanceSlotSchemaV1>,
    ) -> Result<Self, InheritanceSlotSchemaBuildError> {
        for (index, pair) in records.windows(2).enumerate() {
            if pair[0].role() >= pair[1].role() {
                return Err(InheritanceSlotSchemaBuildError::RoleOrder { index: index + 1 });
            }
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[InheritanceSlotSchemaV1] {
        &self.records
    }
    pub fn get(&self, role: InheritanceSlotSchemaRoleV1) -> Option<&InheritanceSlotSchemaV1> {
        self.records
            .binary_search_by_key(&role, InheritanceSlotSchemaV1::role)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalInheritanceSlotSchemasV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceSlotSchemaBuildError {
    DuplicateSlot {
        position: usize,
        slot: PersistentDispatchSlotId,
    },
    RoleOrder {
        index: usize,
    },
}
impl fmt::Display for InheritanceSlotSchemaBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSlot { position, slot } => {
                write!(f, "duplicate slot {slot} at schema position {position}")
            }
            Self::RoleOrder { index } => {
                write!(f, "duplicate or noncanonical schema role at index {index}")
            }
        }
    }
}
impl std::error::Error for InheritanceSlotSchemaBuildError {}

#[cfg(test)]
pub(in crate::cross_cone_type_semantics) mod tests;
