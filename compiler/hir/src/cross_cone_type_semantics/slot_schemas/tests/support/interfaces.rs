use super::*;
use crate::{CanonicalPersistentIdsV1, InterfaceSourceMemberV1};

impl Fixture {
    pub fn interface_source(
        &mut self,
        owner: Node,
        parents: &[Node],
        members: &[(PersistentDispatchSlotId, &[PersistentDispatchSlotId])],
    ) {
        self.interface_sources.insert(
            owner.exact,
            (
                parents.iter().map(|node| node.exact).collect(),
                members
                    .iter()
                    .map(|(slot, overrides)| {
                        InterfaceSourceMemberV1::new(
                            *slot,
                            CanonicalPersistentIdsV1::try_new(overrides.to_vec()).unwrap(),
                        )
                    })
                    .collect(),
            ),
        );
    }
    pub fn declare(&mut self, owner: Node, slot: PersistentDispatchSlotId) {
        if let Some((_, members)) = self.interface_sources.get_mut(&owner.exact) {
            members.push(InterfaceSourceMemberV1::new(
                slot,
                CanonicalPersistentIdsV1::empty(),
            ));
        }
    }
}
