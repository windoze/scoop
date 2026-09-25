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
    ) -> Result<Vec<PersistentExactTypeId>, MirDispatchSchemaError> {
        let ty = self.type_export(exact)?;
        let mut values = reserve(ty.base_and_interfaces().interfaces.len() + 2)?;
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
                values.swap(cursor, cursor - 1);
                cursor -= 1;
            }
        }
        values.dedup();

        Ok(values)
    }

    /// BFS with canonical neighbor order selects the shortest path and then
    /// the lexicographically smallest complete exact-id sequence.
    pub fn canonical_receiver_path(
        &self,
        owner: PersistentExactTypeId,
        receiver: PersistentExactTypeId,
    ) -> Result<Vec<PersistentExactTypeId>, MirDispatchSchemaError> {
        let source = self.type_export(receiver)?;
        let backing = self.type_export(owner)?;
        if let (
            MirTypeOriginV1::SourceNominal(nominal),
            MirTypeRepresentationV1::Object { backing: expected },
            MirTypeOriginV1::GeneratedNominal {
                role: GeneratedNominalKey::ObjectBackingClass { object },
                ..
            },
            MirTypeRepresentationV1::ObjectBacking { .. },
        ) = (
            source.origin(),
            source.representation(),
            backing.origin(),
            backing.representation(),
        ) && nominal == object
            && *expected == owner
        {
            // Two exact views of the same object are not inheritance edges.

            let mut path = reserve(2)?;
            path.extend([owner, receiver]);
            return Ok(path);
        }
        let (nodes, indexes) = self.ancestry(owner)?;
        let mut index = *indexes
            .get(&receiver)
            .ok_or(MirDispatchSchemaError::MissingReceiverPath { owner, receiver })?;
        let mut path = reserve(nodes.len())?;
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

    fn ancestry(&self, owner: PersistentExactTypeId) -> Result<Ancestry, MirDispatchSchemaError> {
        let maximum = self.types.record_count();
        let mut nodes = reserve(maximum)?;
        let mut indexes = HashMap::new();
        scoop_wire::allocation::try_reserve_map(&mut indexes, maximum, &WirePath::root())?;
        self.type_export(owner)?;
        nodes.push((owner, None));
        indexes.insert(owner, 0);
        let mut cursor = 0;
        while cursor < nodes.len() {
            for next in self.neighbors(nodes[cursor].0)? {
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
    ) -> Result<(), MirDispatchSchemaError> {
        if active.contains(&owner) {
            return Err(MirDispatchSchemaError::InheritanceCycle { exact: owner });
        }
        if done.contains(&owner) {
            return Ok(());
        }
        active.push(owner);
        for next in self.neighbors(owner)? {
            self.check_cycles(next, active, done)?;
        }
        active.pop();
        done.insert(owner);
        Ok(())
    }

    pub(in crate::cross_cone_type_bridge) fn validate_with_dependencies(
        &self,
        table: &CanonicalMirDispatchSchemasV1,
        dependencies: &[&CanonicalMirDispatchSchemasV1],
    ) -> Result<(), MirDispatchSchemaError> {
        if dependencies.is_empty() {
            return self.validate_table(table, table);
        }
        let count = dependencies
            .len()
            .checked_add(1)
            .ok_or(MirTypeBridgeLookupError::RecordCountOverflow)?;
        let mut tables = reserve(count)?;
        tables.push(table);
        tables.extend_from_slice(dependencies);
        let lookup = MirTypeBridgeSchemaIndexV1::try_new(&tables)?;
        self.validate_table(table, &lookup)
    }

    fn validate_table(
        &self,
        table: &CanonicalMirDispatchSchemasV1,
        schemas: &dyn MirTypeBridgeSchemaLookupV1,
    ) -> Result<(), MirDispatchSchemaError> {
        let mut active = reserve(self.types.record_count())?;
        let mut done = HashSet::new();
        scoop_wire::allocation::try_reserve_set(
            &mut done,
            self.types.record_count(),
            &WirePath::root(),
        )?;
        for record in table.records() {
            self.check_cycles(record.owner(), &mut active, &mut done)?;
        }
        for record in table.records() {
            self.validate_record(record)?;
            let ty = self.type_export(record.owner())?;
            if let MirBaseClassV1::Base(base) = ty.base_and_interfaces().base {
                let base = schemas
                    .get(base)
                    .ok_or(MirDispatchSchemaError::MissingSchema { owner: base })?;
                let prefix = base.vtable().entries();

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
            let (reachable, _) = self.ancestry(record.owner())?;
            let mut expected = reserve(reachable.len())?;
            for (exact, _) in reachable {
                if matches!(
                    self.type_export(exact)?.representation(),
                    MirTypeRepresentationV1::Interface
                ) && (!interface_owner || exact == record.owner())
                {
                    expected.push(exact);
                }
            }

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
                    self.interface_prefix(record, itable, schemas)?;
                } else {
                    let provider = schemas.get(itable.interface()).ok_or(
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
        table: &dyn MirTypeBridgeSchemaLookupV1,
    ) -> Result<(), MirDispatchSchemaError> {
        let mut inherited = HashSet::new();
        scoop_wire::allocation::try_reserve_set(
            &mut inherited,
            own.entries().len(),
            &WirePath::root(),
        )?;
        let mut positions = HashMap::new();
        scoop_wire::allocation::try_reserve_map(
            &mut positions,
            own.entries().len(),
            &WirePath::root(),
        )?;
        for (position, entry) in own.entries().iter().enumerate() {
            positions.insert(entry.slot(), position);
        }
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
                let Some(&position) = positions.get(&entry.slot()) else {
                    // HIR owns the complete typed override suppression proof.
                    continue;
                };
                if position < cursor
                    || !super::validation::same_non_receiver(
                        own.entries()[position].signature(),
                        entry.signature(),
                    )
                {
                    return Err(MirDispatchSchemaError::InterfaceOrder {
                        owner: record.owner(),
                        interface: *interface,
                    });
                }
                inherited.insert(entry.slot());
                cursor = position + 1;
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
