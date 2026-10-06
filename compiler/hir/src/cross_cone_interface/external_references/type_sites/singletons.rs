//! Actual singleton reads in the shared expression occurrence stream.

use super::*;
use crate::{CanonicalExternalHirReferencesV1, ExternalHirTargetV1, SourceNominalId};
use scoop_identity::{
    ConeIdentity, ExactTypeKey, PersistentObjectValueId, PersistentTypeId, SourceDeclarationKey,
    ValidatedIdentityGraph,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirSingletonUseV1 {
    pub position: ExecutableExpressionPosition,
    pub provider: ConeIdentity,
    pub owner: PersistentTypeId,
    pub value: PersistentObjectValueId,
    pub exact: PersistentExactTypeId,
}

impl HirSingletonUseV1 {
    pub fn initialization_root(
        &self,
        identities: &ValidatedIdentityGraph,
        local_units: &[PersistentInitializationUnitId],
    ) -> Result<Option<PersistentInitializationUnitId>, crate::HirInitializationUseError> {
        crate::initialization_dependencies::initializer_root(
            self.position.root,
            identities,
            local_units,
        )
    }
}

impl CanonicalExternalHirReferencesV1 {
    pub fn materialized_singleton_uses(
        &self,
        current: ConeIdentity,
        identities: &ValidatedIdentityGraph,
    ) -> Result<Vec<HirSingletonUseV1>, HirDependencyTypeRelationError> {
        use HirDependencyTypeRelationError as Error;
        let mut uses = Vec::new();
        for reference in self.records() {
            for site in reference
                .type_sites()
                .records()
                .iter()
                .filter_map(HirDependencyTypeSiteV1::as_expression)
            {
                if site.role() != HirExpressionTypeRoleV1::SingletonValue {
                    continue;
                }
                let exact = identities
                    .canonical_key::<_, ExactTypeKey>(site.exact())
                    .map_err(|error| Error::Identity(Box::new(error)))?;
                // Generic companions are materialized in the application's
                // ODR group. Their argument types do not own singleton reads.
                let ExactTypeKey::Nominal(actual) = exact.as_ref() else {
                    continue;
                };
                let ExternalHirTargetV1::Nominal(SourceNominalId::Concrete(owner)) =
                    reference.target()
                else {
                    return Err(Error::Target(reference.target()));
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
                let value = PersistentObjectValueId::from_source_object(&source)
                    .map_err(|_| Error::Target(reference.target()))?;
                uses.push(HirSingletonUseV1 {
                    position: site.position(),
                    provider: source.origin(),
                    owner,
                    value,
                    exact: site.exact(),
                });
            }
        }
        Ok(uses)
    }
}
