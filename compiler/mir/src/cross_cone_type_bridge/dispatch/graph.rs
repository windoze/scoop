use std::collections::{HashMap, HashSet};

use super::*;

type Ancestry = (
    Vec<(PersistentExactTypeId, Option<usize>)>,
    HashMap<PersistentExactTypeId, usize>,
);

impl MirDispatchSchemaAuthority<'_> {
    pub(super) fn type_export(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirTypeExportV1, MirDispatchSchemaError> {
        self.types
            .get(exact)
            .ok_or(MirDispatchSchemaError::MissingType { exact })
    }

    fn neighbors(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<PersistentExactTypeId>, MirDispatchSchemaError> {
        let ty = self.type_export(exact)?;
        let mut values = reserve(ty.base_and_interfaces().interfaces.len() + 2, meter)?;
        values.extend(ty.base_and_interfaces().interfaces.iter().copied());
        if let MirBaseClassV1::Base(base) = ty.base_and_interfaces().base {
            values.push(base);
        }
        if let MirTypeRepresentationV1::Object { backing } = ty.representation() {
            values.push(*backing);
        }
        // Each input interface set is canonical. Inserting at most two
        // additional edges keeps ordering linear in the number of edges.
        for index in 1..values.len() {
            let mut cursor = index;
            while cursor > 0 && values[cursor] < values[cursor - 1] {
                meter.charge_work(1, &WirePath::root())?;
                values.swap(cursor, cursor - 1);
                cursor -= 1;
            }
        }
        values.dedup();
        meter.charge_edges(values.len() as u64, &WirePath::root())?;
        Ok(values)
    }

    /// BFS with canonical neighbor order selects the shortest path and then
    /// the lexicographically smallest complete exact-id sequence.
    pub fn canonical_receiver_path(
        &self,
        owner: PersistentExactTypeId,
        receiver: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<PersistentExactTypeId>, MirDispatchSchemaError> {
        let (nodes, indexes) = self.ancestry(owner, meter)?;
        let mut index = *indexes
            .get(&receiver)
            .ok_or(MirDispatchSchemaError::MissingReceiverPath { owner, receiver })?;
        let mut path = reserve(nodes.len(), meter)?;
        loop {
            path.push(nodes[index].0);
            if let Some(parent) = nodes[index].1 {
                index = parent;
            } else {
                break;
            }
        }
        path.reverse();
        Ok(path)
    }

    fn ancestry(
        &self,
        owner: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Ancestry, MirDispatchSchemaError> {
        let maximum = self.types.records().len();
        let mut nodes = reserve(maximum, meter)?;
        let mut indexes = HashMap::new();
        meter.try_reserve_map_slots(&mut indexes, maximum, &WirePath::root())?;
        self.type_export(owner)?;
        nodes.push((owner, None));
        indexes.insert(owner, 0);
        let mut cursor = 0;
        while cursor < nodes.len() {
            meter.charge_work(1, &WirePath::root())?;
            for next in self.neighbors(nodes[cursor].0, meter)? {
                self.type_export(next)?;
                if let std::collections::hash_map::Entry::Vacant(entry) = indexes.entry(next) {
                    entry.insert(nodes.len());
                    nodes.push((next, Some(cursor)));
                }
            }
            cursor += 1;
        }
        Ok((nodes, indexes))
    }

    fn check_cycles(
        &self,
        owner: PersistentExactTypeId,
        active: &mut Vec<PersistentExactTypeId>,
        done: &mut HashSet<PersistentExactTypeId>,
        meter: &mut BudgetMeter,
    ) -> Result<(), MirDispatchSchemaError> {
        meter.check_semantic_depth(active.len() as u64, &WirePath::root())?;
        meter.charge_work(active.len() as u64 + 1, &WirePath::root())?;
        if active.contains(&owner) {
            return Err(MirDispatchSchemaError::InheritanceCycle { exact: owner });
        }
        if done.contains(&owner) {
            return Ok(());
        }
        active.push(owner);
        for next in self.neighbors(owner, meter)? {
            self.check_cycles(next, active, done, meter)?;
        }
        active.pop();
        done.insert(owner);
        Ok(())
    }

    pub(super) fn validate_table(
        &self,
        table: &CanonicalMirDispatchSchemasV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), MirDispatchSchemaError> {
        let mut active = reserve(self.types.records().len(), meter)?;
        let mut done = HashSet::new();
        meter.try_reserve_set_slots(&mut done, self.types.records().len(), &WirePath::root())?;
        for record in table.records() {
            self.check_cycles(record.owner(), &mut active, &mut done, meter)?;
        }
        for record in table.records() {
            self.validate_record(record, meter)?;
            let ty = self.type_export(record.owner())?;
            if let MirBaseClassV1::Base(base) = ty.base_and_interfaces().base {
                let base = table
                    .get(base)
                    .ok_or(MirDispatchSchemaError::MissingSchema { owner: base })?;
                let prefix = base.vtable().entries();
                meter.charge_work(prefix.len() as u64, &WirePath::root())?;
                if !prefix
                    .iter()
                    .zip(record.vtable().entries())
                    .all(|(left, right)| same_slot(left, right))
                    || record.vtable().entries().len() < prefix.len()
                {
                    return Err(MirDispatchSchemaError::BasePrefix {
                        owner: record.owner(),
                    });
                }
            }
            let interface_owner = matches!(ty.representation(), MirTypeRepresentationV1::Interface);
            let (reachable, _) = self.ancestry(record.owner(), meter)?;
            let mut expected = reserve(reachable.len(), meter)?;
            for (exact, _) in reachable {
                if matches!(
                    self.type_export(exact)?.representation(),
                    MirTypeRepresentationV1::Interface
                ) && (!interface_owner || exact == record.owner())
                {
                    expected.push(exact);
                }
            }
            charge_sort(expected.len(), meter)?;
            expected.sort_unstable();
            if !record
                .itables()
                .iter()
                .map(MirInterfaceDispatchTableV1::interface)
                .eq(expected)
            {
                return Err(MirDispatchSchemaError::InterfaceClosure {
                    owner: record.owner(),
                });
            }
            for itable in record.itables() {
                if itable.interface() == record.owner() {
                    self.interface_prefix(record, itable, table, meter)?;
                } else {
                    let provider = table.get(itable.interface()).ok_or(
                        MirDispatchSchemaError::MissingSchema {
                            owner: itable.interface(),
                        },
                    )?;
                    let expected = provider.interface_table(itable.interface()).ok_or(
                        MirDispatchSchemaError::MissingInterfaceTable {
                            owner: provider.owner(),
                            interface: itable.interface(),
                        },
                    )?;
                    meter.charge_work(itable.entries().len() as u64, &WirePath::root())?;
                    if itable.entries().len() != expected.entries().len()
                        || !itable
                            .entries()
                            .iter()
                            .zip(expected.entries())
                            .all(|(left, right)| same_slot(left, right))
                    {
                        return Err(MirDispatchSchemaError::InterfaceOrder {
                            owner: record.owner(),
                            interface: itable.interface(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn interface_prefix(
        &self,
        record: &ParamFreeMirDispatchSchemaV1,
        own: &MirInterfaceDispatchTableV1,
        table: &CanonicalMirDispatchSchemasV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), MirDispatchSchemaError> {
        let mut inherited = HashSet::new();
        meter.try_reserve_set_slots(&mut inherited, own.entries().len(), &WirePath::root())?;
        for interface in &self
            .type_export(record.owner())?
            .base_and_interfaces()
            .interfaces
        {
            let provider = table
                .get(*interface)
                .ok_or(MirDispatchSchemaError::MissingSchema { owner: *interface })?;
            let parent = provider.interface_table(*interface).ok_or(
                MirDispatchSchemaError::MissingInterfaceTable {
                    owner: *interface,
                    interface: *interface,
                },
            )?;
            let mut cursor = 0;
            for entry in parent.entries() {
                meter.charge_work(1, &WirePath::root())?;
                while cursor < own.entries().len() && own.entries()[cursor].slot() != entry.slot() {
                    meter.charge_work(1, &WirePath::root())?;
                    cursor += 1;
                }
                if cursor == own.entries().len()
                    || !super::validation::same_non_receiver(
                        own.entries()[cursor].signature(),
                        entry.signature(),
                    )
                {
                    return Err(MirDispatchSchemaError::InterfaceOrder {
                        owner: record.owner(),
                        interface: *interface,
                    });
                }
                inherited.insert(entry.slot());
                cursor += 1;
            }
        }
        if own
            .entries()
            .iter()
            .take(inherited.len())
            .any(|entry| !inherited.contains(&entry.slot()))
        {
            return Err(MirDispatchSchemaError::InterfaceOrder {
                owner: record.owner(),
                interface: record.owner(),
            });
        }
        Ok(())
    }
}

fn same_slot(left: &MirDispatchEntryV1, right: &MirDispatchEntryV1) -> bool {
    left.slot() == right.slot()
        && left.position() == right.position()
        && left.signature() == right.signature()
}
