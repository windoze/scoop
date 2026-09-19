use std::fmt;

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{Encoder, WireEncode};

use crate::PersistentSlotContractDomainV1;
use crate::cross_cone_type_semantics::wire;

mod decode;
#[cfg(test)]
mod tests;

pub use decode::*;

/// A claimed slot call region, without source-slot or access authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedDefaultSlotCallDomainV1 {
    slot: PersistentDispatchSlotId,
    domain: PersistentSlotContractDomainV1,
}

impl ProtectedDefaultSlotCallDomainV1 {
    pub const fn new(
        slot: PersistentDispatchSlotId,
        domain: PersistentSlotContractDomainV1,
    ) -> Self {
        Self { slot, domain }
    }
    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }
    pub const fn domain(&self) -> &PersistentSlotContractDomainV1 {
        &self.domain
    }
}

impl WireEncode for ProtectedDefaultSlotCallDomainV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(2)?;
        self.domain.encode(encoder)
    }
}

/// Empty data does not prove that a callable has no dispatch roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalProtectedDefaultSlotCallDomainsV1 {
    records: Vec<ProtectedDefaultSlotCallDomainV1>,
}

impl CanonicalProtectedDefaultSlotCallDomainsV1 {
    pub fn try_new(
        mut records: Vec<ProtectedDefaultSlotCallDomainV1>,
    ) -> Result<Self, ProtectedDefaultSlotCallDomainsBuildError> {
        u32::try_from(records.len())
            .map_err(|_| ProtectedDefaultSlotCallDomainsBuildError::TooMany)?;
        records.sort_unstable_by_key(ProtectedDefaultSlotCallDomainV1::slot);
        for (index, pair) in records.windows(2).enumerate() {
            validate_pair(&pair[0], &pair[1], index + 1)?;
        }
        Ok(Self { records })
    }
    pub fn records(&self) -> &[ProtectedDefaultSlotCallDomainV1] {
        &self.records
    }
    pub fn get(&self, slot: PersistentDispatchSlotId) -> Option<&ProtectedDefaultSlotCallDomainV1> {
        self.records
            .binary_search_by_key(&slot, ProtectedDefaultSlotCallDomainV1::slot)
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalProtectedDefaultSlotCallDomainsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

fn validate_pair(
    previous: &ProtectedDefaultSlotCallDomainV1,
    next: &ProtectedDefaultSlotCallDomainV1,
    index: usize,
) -> Result<(), ProtectedDefaultSlotCallDomainsBuildError> {
    match previous.slot.cmp(&next.slot) {
        std::cmp::Ordering::Equal => Err(ProtectedDefaultSlotCallDomainsBuildError::Duplicate {
            index,
            slot: next.slot,
        }),
        std::cmp::Ordering::Greater => {
            Err(ProtectedDefaultSlotCallDomainsBuildError::NonCanonicalOrder { index })
        }
        std::cmp::Ordering::Less => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedDefaultSlotCallDomainsBuildError {
    TooMany,
    Duplicate {
        index: usize,
        slot: PersistentDispatchSlotId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl fmt::Display for ProtectedDefaultSlotCallDomainsBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => f.write_str("too many protected default slot call domains"),
            Self::Duplicate { index, .. } => write!(
                f,
                "duplicate protected default slot call domain at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                f,
                "noncanonical protected default slot call domain order at index {index}"
            ),
        }
    }
}

impl std::error::Error for ProtectedDefaultSlotCallDomainsBuildError {}
