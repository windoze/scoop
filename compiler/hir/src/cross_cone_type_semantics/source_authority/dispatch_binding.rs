//! Artifact-owned identity and source joins for the independent dispatch tables.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    DispatchSlotKey, PersistentDispatchSlotId, PersistentExactTypeId, PersistentFunctionId,
    PersistentPropertyAccessorId, PersistentPropertyId, PropertyAccessorKey, SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::binding_keys;
use crate::*;

mod contracts;
mod errors;
mod inventory;
mod keys;
mod replay;
pub use errors::*;

/// Joins source tables to their owning artifact. This is neither a candidate
/// interface nor a complete inheritance or machine-use proof.
#[derive(Debug)]
pub struct BoundInheritanceDispatchSourcesV1<'a, 'f> {
    foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    inventory: &'a CanonicalSourceInheritanceInventoriesV1,
    interfaces: &'a CanonicalInterfaceSourceDispatchesV1,
    selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
    callables: &'a CanonicalInheritanceSourceCallablesV1,
    functions: BTreeMap<PersistentFunctionId, &'f SourceDeclarationKey>,
    properties: BTreeMap<PersistentPropertyId, &'f SourceDeclarationKey>,
    slots: BTreeMap<PersistentDispatchSlotId, &'f DispatchSlotKey>,
}

impl<'f> BoundTypeFoundationSourcesV1<'f> {
    pub fn bind_inheritance_dispatch_sources<'a>(
        &'a self,
        inventory: &'a CanonicalSourceInheritanceInventoriesV1,
        interfaces: &'a CanonicalInterfaceSourceDispatchesV1,
        selections: &'a CanonicalInheritanceSourceSlotSelectionsV1,
        callables: &'a CanonicalInheritanceSourceCallablesV1,
        meter: &mut BudgetMeter,
    ) -> Result<BoundInheritanceDispatchSourcesV1<'a, 'f>, InheritanceDispatchBindingError> {
        meter.check_semantic_depth(1, &WirePath::root())?;
        meter.charge_nodes(1, &WirePath::root())?;
        let canonical = self.foundation.as_canonical();
        let result = BoundInheritanceDispatchSourcesV1 {
            foundation: self,
            inventory,
            interfaces,
            selections,
            callables,
            functions: keys::index(canonical.type_source_function_records(), self, meter)?,
            properties: keys::index(canonical.type_source_property_records(), self, meter)?,
            slots: keys::index(canonical.type_source_dispatch_records(), self, meter)?,
        };
        inventory::validate(&result, meter)?;
        contracts::validate(&result, meter)?;
        Ok(result)
    }
}

impl<'a> BoundInheritanceDispatchSourcesV1<'a, '_> {
    pub const fn inventory(&self) -> &'a CanonicalSourceInheritanceInventoriesV1 {
        self.inventory
    }

    pub fn callable(
        &self,
        declaration: InheritanceCallableDeclarationV1,
    ) -> Result<InheritanceSourceCallableFactsV1<'a>, InheritanceDispatchBindingError> {
        self.callables
            .get(declaration)
            .map(InheritanceSourceCallableV1::facts)
            .ok_or(InheritanceDispatchBindingError::MissingCallable(
                declaration,
            ))
    }

    pub fn selection(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    ) -> Result<InheritanceSourceSlotSelectionV1, InheritanceDispatchBindingError> {
        self.selections
            .get(owner, slot)
            .ok_or(InheritanceDispatchBindingError::MissingSelection { owner, slot })
    }
}

fn charge(count: usize, meter: &mut BudgetMeter) -> Result<(), InheritanceDispatchBindingError> {
    binding_keys::charge_map(count, meter, &WirePath::root())?;
    Ok(())
}

fn charge_queries(
    count: usize,
    index_length: usize,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceDispatchBindingError> {
    meter.charge_work(
        (count as u64).saturating_mul(u64::from(index_length.max(1).ilog2()) + 1),
        &WirePath::root(),
    )?;
    Ok(())
}
