//! Constructor source contracts bound to artifact-owned identity and origins.

use std::collections::BTreeMap;

use scoop_identity::{
    ConeIdentity, PersistentConstructorId, PersistentExactTypeId, SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::binding_keys;
use crate::*;

mod contracts;
mod errors;
mod inventory;
pub use errors::*;

/// Provides artifact-bound sources, not construction permission, default
/// expansion, a complete source-body proof, or a machine-use capability.
#[derive(Debug)]
pub struct BoundInheritanceConstructorSourcesV1<'a, 'f> {
    pub(super) foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    pub(super) inventory: &'a CanonicalSourceInheritanceInventoriesV1,
    constructors: &'a CanonicalInheritanceSourceConstructorsV1,
    keys: BTreeMap<PersistentConstructorId, &'f SourceDeclarationKey>,
}

impl<'f> BoundTypeFoundationSourcesV1<'f> {
    pub fn bind_inheritance_constructor_sources<'a>(
        &'a self,
        inventory: &'a CanonicalSourceInheritanceInventoriesV1,
        constructors: &'a CanonicalInheritanceSourceConstructorsV1,
        meter: &mut BudgetMeter,
    ) -> Result<BoundInheritanceConstructorSourcesV1<'a, 'f>, InheritanceConstructorBindingError>
    {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        let owners = inventory::required(self, inventory, constructors, meter)?;
        let available = binding_keys::index(
            self.foundation
                .as_canonical()
                .type_source_constructor_records(),
            meter,
            &path,
        )?;
        let mut keys = BTreeMap::new();
        binding_keys::charge_map(constructors.records().len(), meter, &path)?;
        for record in constructors.records() {
            charge_queries(1, available.len(), meter)?;
            let declaration = record.declaration();
            let key = available
                .get(&declaration)
                .copied()
                .ok_or(InheritanceConstructorBindingError::MissingKey(declaration))?;
            binding_keys::verify(declaration, key, self.identities, meter, &path)?;
            contracts::validate(self, owners[&declaration], key, record, meter)?;
            keys.insert(declaration, key);
        }
        Ok(BoundInheritanceConstructorSourcesV1 {
            foundation: self,
            inventory,
            constructors,
            keys,
        })
    }
}

impl<'a, 'f> BoundInheritanceConstructorSourcesV1<'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.foundation.source().entries().provider
    }

    pub const fn table(&self) -> &'a CanonicalInheritanceSourceConstructorsV1 {
        self.constructors
    }

    pub fn required_for(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<
        &'a CanonicalPersistentIdsV1<PersistentConstructorId>,
        InheritanceConstructorBindingError,
    > {
        self.inventory
            .get(owner)
            .map(SourceInheritanceInventoryV1::constructors)
            .ok_or(InheritanceConstructorBindingError::MissingOwner(owner))
    }

    pub fn constructor_key(
        &self,
        declaration: PersistentConstructorId,
    ) -> Result<&'f SourceDeclarationKey, InheritanceConstructorBindingError> {
        self.keys
            .get(&declaration)
            .copied()
            .ok_or(InheritanceConstructorBindingError::MissingKey(declaration))
    }

    pub fn constructor_source(
        &self,
        declaration: PersistentConstructorId,
    ) -> Result<&'a NominalSupportConstructorInterfaceV1, InheritanceConstructorBindingError> {
        self.constructors
            .get(declaration)
            .ok_or(InheritanceConstructorBindingError::MissingSource(
                declaration,
            ))
    }
}

fn charge_queries(
    count: usize,
    length: usize,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceConstructorBindingError> {
    meter.charge_work(
        (count as u64).saturating_mul(u64::from(length.max(1).ilog2()) + 1),
        &WirePath::root(),
    )?;
    Ok(())
}
