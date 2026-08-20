use std::collections::BTreeSet;

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{WireError, WirePath};

use crate::InterfaceSourceMemberV1;

/// Applies the declaration's already selected override relations in order.
#[derive(Clone, Default)]
pub(crate) struct InterfaceSlotExpansion {
    sequence: Vec<PersistentDispatchSlotId>,
    suppressed: BTreeSet<PersistentDispatchSlotId>,
}

impl InterfaceSlotExpansion {
    pub fn inherit<'a>(parents: impl IntoIterator<Item = &'a Self>) -> Result<Self, WireError> {
        let mut expansion = Self::default();
        let mut inherited = BTreeSet::new();
        for parent in parents {
            for slot in &parent.sequence {
                if inherited.insert(*slot) {
                    expansion.push(*slot)?;
                }
            }
            expansion.suppressed.extend(&parent.suppressed);
        }
        Ok(expansion)
    }

    pub fn sequence(&self) -> &[PersistentDispatchSlotId] {
        &self.sequence
    }

    pub fn declare(&mut self, member: &InterfaceSourceMemberV1) -> Result<(), WireError> {
        self.suppressed.extend(member.overrides().values());
        self.push(member.slot())
    }

    pub fn slots(&self) -> impl Iterator<Item = PersistentDispatchSlotId> + '_ {
        self.sequence
            .iter()
            .filter(|slot| !self.suppressed.contains(slot))
            .copied()
    }

    fn push(&mut self, slot: PersistentDispatchSlotId) -> Result<(), WireError> {
        scoop_wire::allocation::try_reserve(&mut self.sequence, 1, &WirePath::root())?;
        self.sequence.push(slot);
        Ok(())
    }
}
