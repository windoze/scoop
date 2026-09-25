//! Complete nominal constructors joined to artifact-owned source declarations.
use super::binding_keys;
use crate::*;
use scoop_identity::{ConeIdentity, PersistentConstructorId, SourceDeclarationKey};
use scoop_wire::{WireError, WirePath};
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
    ) -> Result<BoundNominalConstructorSourcesV1<'s, 'a, 'f>, Error> {
        let mut required = BTreeMap::new();
        for nominal in self.table().records() {
            let constructors = nominal.constructors().values();

            for declaration in constructors {
                if required.insert(*declaration, nominal).is_some() {
                    return Err(Error::Inventory);
                }
            }
        }

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
        )?;

        let mut keys = BTreeMap::new();
        for record in table.records() {
            let declaration = record.declaration();
            let key = available
                .get(&declaration)
                .copied()
                .ok_or(Error::MissingKey(declaration))?;
            binding_keys::verify(declaration, key, self.foundation.identities)?;
            contracts::validate(self, required[&declaration], key, record)?;
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

fn invalid(declaration: PersistentConstructorId, reason: impl ToString) -> Error {
    Error::Contract {
        declaration,
        reason: reason.to_string(),
    }
}
