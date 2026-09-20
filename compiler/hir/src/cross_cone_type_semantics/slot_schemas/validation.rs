use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    DispatchSlotKey, PersistentDispatchSlotId, PersistentExactTypeId, PersistentFunctionId,
    PersistentPropertyAccessorId, PersistentPropertyId, PropertyAccessorKey, SourceDeclarationKey,
    SourceDeclarationKind,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{CanonicalInheritanceSlotSchemasV1, InheritanceSlotSchemaRoleV1 as Role};
use crate::{CheckedNominalInheritanceGraphV1, DirectClassBaseV1, InheritanceQueryError};

mod error;
mod identity;
mod interfaces;
pub use error::*;

/// Canonical foundation keys and schema records from the same explicit graph.
/// This establishes slot identity and semantic ordering, not callable effects,
/// implementation selection, default coverage, or source lookup permission.
pub trait InheritanceSlotSchemaSemanticAuthority<E> {
    fn interface_dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&crate::InterfaceSourceDispatchV1, E>;
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, E>;
    fn dispatch_slot_key(&self, slot: PersistentDispatchSlotId) -> Result<&DispatchSlotKey, E>;
    fn function_key(&self, function: PersistentFunctionId) -> Result<&SourceDeclarationKey, E>;
    fn accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, E>;
    fn property_key(&self, property: PersistentPropertyId) -> Result<&SourceDeclarationKey, E>;
}

#[derive(Clone, Copy, Debug)]
pub struct CheckedInheritanceSlotSchemasV1<'a> {
    owner: PersistentExactTypeId,
    schemas: &'a CanonicalInheritanceSlotSchemasV1,
}
impl CheckedInheritanceSlotSchemasV1<'_> {
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub const fn schemas(&self) -> &CanonicalInheritanceSlotSchemasV1 {
        self.schemas
    }
}

impl CheckedNominalInheritanceGraphV1<'_> {
    pub fn validate_slot_schemas<'a, A: InheritanceSlotSchemaSemanticAuthority<E>, E>(
        &self,
        owner: PersistentExactTypeId,
        authority: &'a A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedInheritanceSlotSchemasV1<'a>, InheritanceSlotSchemaSemanticError<E>> {
        let mut validation = Validation {
            graph: self,
            authority,
            meter,
            complete: BTreeMap::new(),
            conformances: BTreeMap::new(),
            interface_expansions: BTreeMap::new(),
        };
        validation.visit(owner, 1)?;
        Ok(CheckedInheritanceSlotSchemasV1 {
            owner,
            schemas: validation.complete[&owner],
        })
    }
}

struct Validation<'a, 'g, 'w, A> {
    graph: &'g CheckedNominalInheritanceGraphV1<'w>,
    authority: &'a A,
    meter: &'g mut BudgetMeter,
    complete: BTreeMap<PersistentExactTypeId, &'a CanonicalInheritanceSlotSchemasV1>,
    conformances: BTreeMap<PersistentExactTypeId, BTreeSet<PersistentExactTypeId>>,
    interface_expansions: BTreeMap<PersistentExactTypeId, interfaces::Expansion>,
}
impl<A> Validation<'_, '_, '_, A> {
    fn visit<E>(
        &mut self,
        owner: PersistentExactTypeId,
        depth: u64,
    ) -> Result<(), InheritanceSlotSchemaSemanticError<E>>
    where
        A: InheritanceSlotSchemaSemanticAuthority<E>,
    {
        let path = WirePath::root();
        self.meter
            .charge_work(1, &path)
            .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
        self.meter
            .check_semantic_depth(depth, &path)
            .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
        if self.complete.contains_key(&owner) {
            return Ok(());
        }
        let node = self
            .graph
            .get(owner)
            .ok_or(InheritanceSlotSchemaSemanticError::Inheritance(
                InheritanceQueryError::UnknownExact(owner),
            ))?;
        let kind = self
            .graph
            .source(node.source())
            .ok_or(InheritanceSlotSchemaSemanticError::Inheritance(
                InheritanceQueryError::UnknownSource(node.source()),
            ))?
            .key
            .declaration_kind();
        let mut interfaces = BTreeSet::new();
        if let DirectClassBaseV1::ClassBase { exact } = node.edges().direct_base() {
            self.visit(exact, depth + 1)?;
            self.add_interfaces(exact, &mut interfaces)?;
        }
        for interface in node.edges().direct_interfaces() {
            self.visit(*interface, depth + 1)?;
            self.add_interfaces(*interface, &mut interfaces)?;
        }
        if kind == SourceDeclarationKind::Interface {
            self.meter
                .charge_collection_slots(1, &path)
                .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
            interfaces.insert(owner);
        }
        let schemas = self
            .authority
            .schemas(owner)
            .map_err(InheritanceSlotSchemaSemanticError::Foundation)?;
        let class_like = matches!(
            kind,
            SourceDeclarationKind::Class | SourceDeclarationKind::Object
        );
        let expected_count = if kind == SourceDeclarationKind::Interface {
            1
        } else {
            interfaces.len() + usize::from(class_like)
        };
        if schemas.records().len() != expected_count {
            return Err(InheritanceSlotSchemaSemanticError::RoleCoverage(owner));
        }
        for schema in schemas.records() {
            match schema.role() {
                Role::ClassVtable if class_like => {
                    self.vtable(owner, node.edges().direct_base(), schema.slots())?
                }
                Role::Interface { interface_exact }
                    if interfaces.contains(&interface_exact)
                        && (kind != SourceDeclarationKind::Interface
                            || interface_exact == owner) =>
                {
                    if interface_exact == owner {
                        self.interface_schema(
                            owner,
                            node.edges().direct_interfaces(),
                            schema.slots(),
                        )?;
                    } else {
                        let expected = self.complete[&interface_exact]
                            .get(Role::Interface { interface_exact })
                            .ok_or(InheritanceSlotSchemaSemanticError::RoleCoverage(
                                interface_exact,
                            ))?;
                        if schema.slots() != expected.slots() {
                            return Err(InheritanceSlotSchemaSemanticError::InterfaceOrder {
                                owner,
                                interface: interface_exact,
                            });
                        }
                    }
                }
                _ => return Err(InheritanceSlotSchemaSemanticError::RoleCoverage(owner)),
            }
        }
        self.meter
            .charge_nodes(2, &path)
            .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
        self.meter
            .charge_collection_slots(2, &path)
            .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
        self.complete.insert(owner, schemas);
        self.conformances.insert(owner, interfaces);
        Ok(())
    }

    fn add_interfaces<E>(
        &mut self,
        owner: PersistentExactTypeId,
        result: &mut BTreeSet<PersistentExactTypeId>,
    ) -> Result<(), InheritanceSlotSchemaSemanticError<E>> {
        let entries = &self.conformances[&owner];
        self.meter
            .charge_collection_slots(entries.len() as u64, &WirePath::root())
            .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
        self.meter
            .charge_work(entries.len() as u64, &WirePath::root())
            .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
        result.extend(entries);
        Ok(())
    }

    fn vtable<E>(
        &mut self,
        owner: PersistentExactTypeId,
        base: DirectClassBaseV1,
        slots: &[PersistentDispatchSlotId],
    ) -> Result<(), InheritanceSlotSchemaSemanticError<E>>
    where
        A: InheritanceSlotSchemaSemanticAuthority<E>,
    {
        let prefix = if let DirectClassBaseV1::ClassBase { exact } = base {
            self.complete[&exact]
                .get(Role::ClassVtable)
                .ok_or(InheritanceSlotSchemaSemanticError::RoleCoverage(exact))?
                .slots()
        } else {
            &[]
        };
        self.meter
            .charge_work(prefix.len() as u64, &WirePath::root())
            .map_err(InheritanceSlotSchemaSemanticError::Resource)?;
        if !slots.starts_with(prefix) {
            return Err(InheritanceSlotSchemaSemanticError::BasePrefix(owner));
        }
        for slot in &slots[prefix.len()..] {
            let root = identity::source_owner(self.graph, *slot, self.authority, self.meter)?;
            if root != owner {
                return Err(InheritanceSlotSchemaSemanticError::NewSlotOwner {
                    owner,
                    slot: *slot,
                });
            }
            self.graph
                .require_class(root)
                .map_err(InheritanceSlotSchemaSemanticError::Inheritance)?;
        }
        Ok(())
    }
}
