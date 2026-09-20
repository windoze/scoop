//! Source-only dependency composition. Semantic candidates remain untrusted.

use super::*;

mod errors;
mod index;
mod provider;
mod replay;
#[cfg(test)]
mod tests;
pub use errors::*;
pub use provider::*;

/// Independent source inputs for the existing complete type-section validator.
/// Neither construction nor a successful query grants a consumer capability.
#[derive(Debug)]
pub struct TypeFoundationSourceClosureV1<'a> {
    root: TypeFoundationSourceProviderV1<'a>,
    providers: BTreeMap<ConeIdentity, TypeFoundationSourceProviderV1<'a>>,
    exacts: BTreeMap<PersistentExactTypeId, &'a ExactTypeKey>,
    nominals: BTreeMap<SourceNominalId, TypeFoundationSourceProviderV1<'a>>,
    generated: BTreeMap<PersistentTypeId, &'a GeneratedNominalKey>,
    accessors: BTreeMap<PersistentPropertyAccessorId, &'a PropertyAccessorKey>,
    facts: BTreeMap<PersistentExactTypeId, (ConeIdentity, &'a ExactTypeFactShapeV1)>,
}

impl<'a> TypeFoundationSourceClosureV1<'a> {
    pub fn try_new(
        root: TypeFoundationSourceProviderV1<'a>,
        dependencies: &[TypeFoundationSourceProviderV1<'a>],
        meter: &mut BudgetMeter,
    ) -> Result<Self, TypeFoundationReplayError> {
        index::compose(root, dependencies, meter)
    }

    fn nominal(
        &self,
        owner: SourceNominalId,
    ) -> Result<TypeFoundationSourceProviderV1<'a>, TypeFoundationReplayError> {
        self.nominals
            .get(&owner)
            .copied()
            .ok_or_else(|| TypeFoundationBindingError::MissingNominal(owner).into())
    }
}
