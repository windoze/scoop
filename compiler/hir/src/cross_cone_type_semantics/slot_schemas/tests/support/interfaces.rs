use super::*;
use crate::{CanonicalPersistentIdsV1, InterfaceSourceDispatchV1, InterfaceSourceMemberV1};
use scoop_wire::{BudgetMeter, DecodeLimits};

impl Fixture {
    pub fn interface_source(
        &mut self,
        owner: Node,
        parents: &[Node],
        members: &[(PersistentDispatchSlotId, &[PersistentDispatchSlotId])],
    ) {
        let source = InterfaceSourceDispatchV1::try_new(
            owner.exact,
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
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
        .unwrap();
        self.interface_sources.insert(owner.exact, source);
    }
    pub fn declare(&mut self, owner: Node, slot: PersistentDispatchSlotId) {
        if let Some(source) = self.interface_sources.get(&owner.exact) {
            let mut members = source.members().to_vec();
            members.push(InterfaceSourceMemberV1::new(
                slot,
                CanonicalPersistentIdsV1::empty(),
            ));
            let source = InterfaceSourceDispatchV1::try_new(
                owner.exact,
                source.parents().to_vec(),
                members,
                &mut BudgetMeter::new(DecodeLimits::default()),
            )
            .unwrap();
            self.interface_sources.insert(owner.exact, source);
        }
    }
}
