use super::*;

impl<A> Validation<'_, '_, '_, A> {
    pub(super) fn interface_schema<E>(
        &mut self,
        owner: PersistentExactTypeId,
        parents: &[PersistentExactTypeId],
        slots: &[PersistentDispatchSlotId],
    ) -> Result<(), InheritanceSlotSchemaSemanticError<E>>
    where
        A: InheritanceSlotSchemaSemanticAuthority<E>,
    {
        use InheritanceSlotSchemaSemanticError as Error;

        let source_parents = self
            .authority
            .interface_parent_order(owner)
            .map_err(Error::Foundation)?;
        let members = self
            .authority
            .interface_members(owner)
            .map_err(Error::Foundation)?;
        if source_parents.len() != parents.len()
            || source_parents
                .iter()
                .any(|parent| parents.binary_search(parent).is_err())
        {
            return Err(Error::InterfaceSource(owner));
        }
        for slot in slots {
            identity::source_owner(self.graph, *slot, self.authority)?;
        }
        let mut expansion = crate::InterfaceSlotExpansion::inherit(
            source_parents
                .iter()
                .map(|parent| &self.interface_expansions[parent]),
        )
        .map_err(Error::Resource)?;
        let inherited: BTreeSet<_> = expansion.sequence().iter().copied().collect();
        for member in members {
            let slot = member.slot();
            if self.graph.get(owner).map(|node| node.source())
                != Some(identity::source_owner(self.graph, slot, self.authority)?)
            {
                return Err(Error::NewSlotOwner { owner, slot });
            }
            let role = self
                .authority
                .dispatch_slot_key(slot)
                .map_err(Error::Foundation)?
                .role();
            for overridden in member.overrides().values() {
                if !inherited.contains(overridden)
                    || self
                        .authority
                        .dispatch_slot_key(*overridden)
                        .map_err(Error::Foundation)?
                        .role()
                        != role
                {
                    return Err(Error::InvalidOverride {
                        owner,
                        slot,
                        overridden: *overridden,
                    });
                }
            }
            expansion.declare(member).map_err(Error::Resource)?;
        }

        if !expansion.slots().eq(slots.iter().copied()) {
            return Err(Error::InheritedSlots(owner));
        }

        self.interface_expansions.insert(owner, expansion);
        Ok(())
    }
}
