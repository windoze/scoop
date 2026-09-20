//! Complete nominal constructors joined to artifact-owned source declarations.
use super::binding_keys;
use crate::*;
use scoop_identity::{ConeIdentity, PersistentConstructorId, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};
use std::collections::BTreeMap;

mod access;
mod contracts;
mod errors;
mod signatures;
pub use errors::*;
type Error = NominalConstructorBindingError;

/// Source queries only; lookup, default expansion and machine use require the
/// enclosing declarations transaction and its independently checked uses.
#[derive(Debug)]
pub struct BoundNominalConstructorSourcesV1<'s, 'a, 'f> {
    pub(super) nominals: &'s BoundNominalSourceContractsV1<'a, 'f>,
    table: &'s CanonicalNominalSourceConstructorsV1,
    keys: BTreeMap<PersistentConstructorId, &'f SourceDeclarationKey>,
}
impl<'a, 'f> BoundNominalSourceContractsV1<'a, 'f> {
    pub fn bind_constructor_sources<'s>(
        &'s self,
        table: &'s CanonicalNominalSourceConstructorsV1,
        meter: &mut BudgetMeter,
    ) -> Result<BoundNominalConstructorSourcesV1<'s, 'a, 'f>, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(self.table().records().len() as u64, &path)?;
        let mut required = BTreeMap::new();
        for nominal in self.table().records() {
            let constructors = nominal.constructors().values();
            binding_keys::charge_map(constructors.len(), meter, &path)?;
            for declaration in constructors {
                meter.check_table_entries(required.len() as u64 + 1, &path)?;
                query(required.len(), meter)?;
                if required.insert(*declaration, nominal).is_some() {
                    return Err(Error::Inventory);
                }
            }
        }
        meter.charge_work(required.len() as u64, &path)?;
        if !required.keys().copied().eq(table
            .records()
            .iter()
            .map(NominalSupportConstructorInterfaceV1::declaration))
        {
            return Err(Error::Inventory);
        }
        let available = binding_keys::index(
            self.foundation
                .foundation
                .as_canonical()
                .type_source_constructor_records(),
            meter,
            &path,
        )?;
        binding_keys::charge_map(table.records().len(), meter, &path)?;
        let mut keys = BTreeMap::new();
        for record in table.records() {
            query(available.len(), meter)?;
            query(required.len(), meter)?;
            let declaration = record.declaration();
            let key = available
                .get(&declaration)
                .copied()
                .ok_or(Error::MissingKey(declaration))?;
            binding_keys::verify(declaration, key, self.foundation.identities, meter, &path)?;
            contracts::validate(self, required[&declaration], key, record, meter)?;
            keys.insert(declaration, key);
        }
        Ok(BoundNominalConstructorSourcesV1 {
            nominals: self,
            table,
            keys,
        })
    }
}
impl<'s, 'a, 'f> BoundNominalConstructorSourcesV1<'s, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.nominals.foundation.source().entries().provider
    }
    pub const fn table(&self) -> &'s CanonicalNominalSourceConstructorsV1 {
        self.table
    }
    pub fn constructor_key(
        &self,
        declaration: PersistentConstructorId,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        self.keys
            .get(&declaration)
            .copied()
            .ok_or(Error::MissingKey(declaration))
    }
    pub fn constructor_source(
        &self,
        declaration: PersistentConstructorId,
    ) -> Result<&'s NominalSupportConstructorInterfaceV1, Error> {
        self.table
            .get(declaration)
            .ok_or(Error::MissingSource(declaration))
    }
}
fn query(count: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())?;
    Ok(())
}
fn invalid(declaration: PersistentConstructorId, reason: impl ToString) -> Error {
    Error::Contract {
        declaration,
        reason: reason.to_string(),
    }
}
