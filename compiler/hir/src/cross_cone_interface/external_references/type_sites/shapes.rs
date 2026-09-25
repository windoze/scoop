//! Actual shape operations retain their source owner across machine stages.

use crate::{
    CanonicalExternalHirReferencesV1, ExternalHirTargetV1, HirDependencyTypeRelationError,
    SourceNominalId,
};
use scoop_identity::{
    ConeIdentity, ExactTypeKey, PersistentTypeId, SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_wire::WirePath;

impl CanonicalExternalHirReferencesV1 {
    /// Queries actual nominal operations after ordinary occurrence/origin
    /// validation. Structural constituents never own the enclosing shape.
    /// This result is a dependency root, not a machine-definition capability.
    pub fn materialized_shape_dependencies(
        &self,
        current: ConeIdentity,
        identities: &ValidatedIdentityGraph,
    ) -> Result<Vec<(ConeIdentity, PersistentTypeId)>, HirDependencyTypeRelationError> {
        use HirDependencyTypeRelationError as Error;
        let path = WirePath::root().field(10);
        let mut result = Vec::new();
        for reference in self.records() {
            for site in reference.type_sites().records() {
                let Some(site) = site.as_expression() else {
                    continue;
                };
                if !site.role().requires_shape_support() {
                    continue;
                }
                let ExternalHirTargetV1::Nominal(SourceNominalId::Concrete(owner)) =
                    reference.target()
                else {
                    return Err(Error::Target(reference.target()));
                };

                let exact = identities
                    .canonical_key::<_, ExactTypeKey>(site.exact())
                    .map_err(|error| Error::Identity(Box::new(error)))?;
                let ExactTypeKey::Nominal(actual) = exact.as_ref() else {
                    continue;
                };
                if *actual != owner {
                    return Err(Error::Target(reference.target()));
                }
                let source = identities
                    .canonical_key::<_, SourceDeclarationKey>(owner)
                    .map_err(|error| Error::Identity(Box::new(error)))?;
                if source.origin() == current || source.origin() != reference.origin() {
                    return Err(Error::Target(reference.target()));
                }

                scoop_wire::allocation::try_reserve(&mut result, 1, &path)?;
                result.push((source.origin(), owner));
            }
        }

        result.sort_unstable();
        result.dedup();
        Ok(result)
    }
}
