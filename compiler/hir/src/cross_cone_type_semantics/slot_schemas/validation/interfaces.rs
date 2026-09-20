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
        let path = WirePath::root();
        let source = self
            .authority
            .interface_dispatch_source(owner)
            .map_err(Error::Foundation)?;
        let work = source
            .parents()
            .len()
            .saturating_mul(parents.len().max(1).ilog2() as usize + 1);
        self.meter
            .charge_work(work as u64, &path)
            .map_err(Error::Resource)?;
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
            identity::source_owner(self.graph, *slot, self.authority, self.meter)?;
        }
        let mut expansion = Expansion {
            sequence: Vec::new(),
            suppressed: BTreeSet::new(),
        };
        let mut inherited = BTreeSet::new();
        for parent in source.parents() {
            let previous = &self.interface_expansions[parent];
            for slot in &previous.sequence {
                charge_set(self.meter, inherited.len())?;
                if inherited.insert(*slot) {
                    push(self.meter, &mut expansion.sequence, *slot)?;
                }
            }
            for slot in &previous.suppressed {
                charge_set(self.meter, expansion.suppressed.len())?;
                expansion.suppressed.insert(*slot);
            }
        }
        for member in source.members() {
            let slot = member.slot();
            if identity::source_owner(self.graph, slot, self.authority, self.meter)? != owner {
                return Err(Error::NewSlotOwner { owner, slot });
            }
            let role = self
                .authority
                .dispatch_slot_key(slot)
                .map_err(Error::Foundation)?
                .role();
            for overridden in member.overrides().values() {
                charge_set(self.meter, inherited.len())?;
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
                charge_set(self.meter, expansion.suppressed.len())?;
                expansion.suppressed.insert(*overridden);
            }
            push(self.meter, &mut expansion.sequence, slot)?;
        }
        let lookup = expansion.suppressed.len().max(1).ilog2() as u64 + 1;
        self.meter
            .charge_work(
                (expansion.sequence.len() as u64).saturating_mul(lookup),
                &path,
            )
            .map_err(Error::Resource)?;
        if !expansion
            .sequence
            .iter()
            .filter(|slot| !expansion.suppressed.contains(slot))
            .copied()
            .eq(slots.iter().copied())
        {
            return Err(Error::InheritedSlots(owner));
        }
        charge_set(self.meter, self.interface_expansions.len())?;
        self.interface_expansions.insert(owner, expansion);
        Ok(())
    }
}

fn charge_set<E>(
    meter: &mut BudgetMeter,
    length: usize,
) -> Result<(), InheritanceSlotSchemaSemanticError<E>> {
    let path = WirePath::root();
    meter
        .charge_work(u64::from(length.max(1).ilog2()) + 1, &path)
        .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
    meter
        .charge_collection_slots(1, &path)
        .map_err(InheritanceSlotSchemaSemanticError::Resource)
}
fn push<E>(
    meter: &mut BudgetMeter,
    output: &mut Vec<PersistentDispatchSlotId>,
    slot: PersistentDispatchSlotId,
) -> Result<(), InheritanceSlotSchemaSemanticError<E>> {
    let path = WirePath::root();
    meter
        .check_table_entries(output.len() as u64 + 1, &path)
        .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
    meter
        .try_reserve_collection_slots(output, 1, &path)
        .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
    output.push(slot);
    Ok(())
}
