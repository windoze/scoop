use super::*;

pub(super) struct Expansion {
    sequence: Vec<PersistentDispatchSlotId>,
    suppressed: BTreeSet<PersistentDispatchSlotId>,
}

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

        let source = self
            .authority
            .interface_dispatch_source(owner)
            .map_err(Error::Foundation)?;

        if source.owner() != owner
            || source.parents().len() != parents.len()
            || source
                .parents()
                .iter()
                .any(|parent| parents.binary_search(parent).is_err())
        {
            return Err(Error::InterfaceSource(owner));
        }
        for slot in slots {
            identity::source_owner(self.graph, *slot, self.authority)?;
        }
        let mut expansion = Expansion {
            sequence: Vec::new(),
            suppressed: BTreeSet::new(),
        };
        let mut inherited = BTreeSet::new();
        for parent in source.parents() {
            let previous = &self.interface_expansions[parent];
            for slot in &previous.sequence {
                if inherited.insert(*slot) {
                    push(&mut expansion.sequence, *slot)?;
                }
            }
            for slot in &previous.suppressed {
                expansion.suppressed.insert(*slot);
            }
        }
        for member in source.members() {
            let slot = member.slot();
            if identity::source_owner(self.graph, slot, self.authority)? != owner {
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

                expansion.suppressed.insert(*overridden);
            }
            push(&mut expansion.sequence, slot)?;
        }

        if !expansion
            .sequence
            .iter()
            .filter(|slot| !expansion.suppressed.contains(slot))
            .copied()
            .eq(slots.iter().copied())
        {
            return Err(Error::InheritedSlots(owner));
        }

        self.interface_expansions.insert(owner, expansion);
        Ok(())
    }
}

fn push<E>(
    output: &mut Vec<PersistentDispatchSlotId>,
    slot: PersistentDispatchSlotId,
) -> Result<(), InheritanceSlotSchemaSemanticError<E>> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(output, 1, &path)
        .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
    output.push(slot);
    Ok(())
}
