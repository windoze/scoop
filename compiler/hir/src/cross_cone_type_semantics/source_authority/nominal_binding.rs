//! Source nominal structure joined to the owning artifact's identity records.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    DefinitionOriginSubject, EnumVariantFieldKey, EnumVariantIdentityKey, FieldIdentityKey,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
    PersistentObjectValueId, SourceDeclarationKey,
};
use scoop_wire::WireError;

use super::binding_keys;
use crate::*;

mod contracts;
mod errors;
mod field_index;
mod inventory;
mod keys;
mod operations;
pub(super) use operations::Applied;
pub use operations::*;
mod replay;
pub use errors::*;

type Error = NominalSourceBindingError;

/// Independent source queries only. Complete declaration support and execution
/// eligibility remain the responsibility of the enclosing section transaction.
#[derive(Debug)]
pub struct BoundNominalSourceContractsV1<'a, 'f> {
    pub(super) foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    table: &'a CanonicalNominalSourceContractsV1,
    fields: BTreeMap<PersistentFieldId, &'f FieldIdentityKey>,
    variants: BTreeMap<PersistentEnumVariantId, &'f EnumVariantIdentityKey>,
    variant_fields: BTreeMap<PersistentEnumVariantFieldId, &'f EnumVariantFieldKey>,
    objects: BTreeMap<PersistentObjectValueId, &'f SourceDeclarationKey>,
    variant_sources: BTreeMap<PersistentEnumVariantId, VariantSource<'a>>,
}

#[derive(Debug)]
struct VariantSource<'a> {
    shape: &'a EnumSourceVariantV1,
    origin: ExportDefinitionSourceV1,
}

impl<'f> BoundTypeFoundationSourcesV1<'f> {
    pub fn bind_nominal_sources<'a>(
        &'a self,
        table: &'a CanonicalNominalSourceContractsV1,
    ) -> Result<BoundNominalSourceContractsV1<'a, 'f>, Error> {
        let roots = self.source().entries().source_roots.values();

        if !table
            .records()
            .iter()
            .map(NominalSourceContractV1::owner)
            .eq(roots.iter().copied())
        {
            return Err(Error::Inventory("nominal owners"));
        }
        inventory::validate(self, table)?;
        let mut bound = BoundNominalSourceContractsV1 {
            foundation: self,
            table,
            fields: BTreeMap::new(),
            variants: BTreeMap::new(),
            variant_fields: BTreeMap::new(),
            objects: BTreeMap::new(),
            variant_sources: BTreeMap::new(),
        };
        keys::bind(&mut bound)?;
        for record in table.records() {
            contracts::validate(&mut bound, record)?;
        }
        Ok(bound)
    }
}

impl<'a, 'f> BoundNominalSourceContractsV1<'a, 'f> {
    pub const fn table(&self) -> &'a CanonicalNominalSourceContractsV1 {
        self.table
    }

    pub fn nominal_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&'a NominalSourceContractV1, Error> {
        self.table.get(owner).ok_or(Error::MissingSource(owner))
    }

    pub fn nominal_field_key(
        &self,
        field: PersistentFieldId,
    ) -> Result<&'f FieldIdentityKey, Error> {
        self.fields
            .get(&field)
            .copied()
            .ok_or(Error::MissingField(field))
    }

    pub fn enum_variant_key(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&'f EnumVariantIdentityKey, Error> {
        self.variants
            .get(&variant)
            .copied()
            .ok_or(Error::MissingVariant(variant))
    }

    pub fn enum_variant_field_key(
        &self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<&'f EnumVariantFieldKey, Error> {
        self.variant_fields
            .get(&field)
            .copied()
            .ok_or(Error::MissingVariantField(field))
    }

    pub fn object_value_key(
        &self,
        value: PersistentObjectValueId,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        self.objects
            .get(&value)
            .copied()
            .ok_or(Error::MissingObject(value))
    }

    pub fn enum_variant_shape(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&'a EnumSourceVariantV1, Error> {
        self.variant_sources
            .get(&variant)
            .map(|source| source.shape)
            .ok_or(Error::MissingVariant(variant))
    }

    pub fn enum_variant_origin(
        &self,
        variant: PersistentEnumVariantId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        self.variant_sources
            .get(&variant)
            .map(|source| &source.origin)
            .ok_or(Error::MissingVariant(variant))
    }
}

fn invalid(owner: SourceNominalId, reason: impl std::fmt::Display) -> Error {
    Error::Contract {
        owner,
        reason: reason.to_string(),
    }
}
