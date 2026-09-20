use std::collections::BTreeMap;

use scoop_identity::{
    GeneratedNominalKey, PropertyAccessorKey, SourceDeclarationKey, ValidatedIdentityGraph,
};

use super::*;

mod closure;
mod errors;
mod keys;
mod origins;
mod replay;
mod sources;
pub use closure::*;
pub use errors::*;

/// The source-side transcript joined to the identity and origin records of
/// the owning artifact. This is not a complete type-section proof.
#[derive(Debug)]
pub struct BoundTypeFoundationSourcesV1<'a> {
    source: &'a TypeFoundationSourceAuthorityV1,
    foundation: &'a OdrFreeHirFoundation,
    exact_keys: BTreeMap<PersistentExactTypeId, &'a ExactTypeKey>,
    nominal_keys: BTreeMap<SourceNominalId, &'a SourceDeclarationKey>,
    generated_keys: BTreeMap<PersistentTypeId, &'a GeneratedNominalKey>,
    accessor_keys: BTreeMap<PersistentPropertyAccessorId, &'a PropertyAccessorKey>,
}

impl TypeFoundationSourceAuthorityV1 {
    pub fn bind_to_foundation<'a>(
        &'a self,
        foundation: &'a OdrFreeHirFoundation,
        identities: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<BoundTypeFoundationSourcesV1<'a>, TypeFoundationBindingError> {
        let result = keys::bind(self, foundation, identities, meter)?;
        origins::validate_all(&result, meter)?;
        sources::validate_all(&result, meter)?;
        Ok(result)
    }
}

impl<'a> BoundTypeFoundationSourcesV1<'a> {
    pub const fn source(&self) -> &'a TypeFoundationSourceAuthorityV1 {
        self.source
    }

    pub fn exact_type_key(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&'a ExactTypeKey, TypeFoundationBindingError> {
        self.exact_keys
            .get(&exact)
            .copied()
            .ok_or(TypeFoundationBindingError::MissingExact(exact))
    }

    pub fn nominal_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&'a SourceDeclarationKey, TypeFoundationBindingError> {
        self.nominal_keys
            .get(&owner)
            .copied()
            .ok_or(TypeFoundationBindingError::MissingNominal(owner))
    }

    pub fn nominal_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&'a TypeSourceNominalV1, TypeFoundationBindingError> {
        self.source
            .entries()
            .sources
            .get(owner)
            .ok_or(TypeFoundationBindingError::MissingNominal(owner))
    }

    pub fn generated_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&'a GeneratedNominalKey, TypeFoundationBindingError> {
        self.generated_keys
            .get(&owner)
            .copied()
            .ok_or(TypeFoundationBindingError::MissingGenerated(owner))
    }

    pub fn accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&'a PropertyAccessorKey, TypeFoundationBindingError> {
        self.accessor_keys
            .get(&accessor)
            .copied()
            .ok_or(TypeFoundationBindingError::MissingAccessor(accessor))
    }

    pub fn contains_definition_source(&self, source: &ExportDefinitionSourceV1) -> bool {
        self.source.entries().definition_sources.contains(source)
    }
}
