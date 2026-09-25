//! Actual shape operations retain their source owner across machine stages.

use crate::{
    CanonicalExternalHirReferencesV1, ExternalHirTargetV1, HirDependencyTypeRelationError,
    SourceNominalId,
};
use scoop_identity::{
    ConeIdentity, ExactTypeKey, PersistentTypeId, SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WirePath};

impl CanonicalExternalHirReferencesV1 {
    /// Queries actual nominal operations after ordinary occurrence/origin
    /// validation. Structural constituents never own the enclosing shape.
    /// This result is a dependency root, not a machine-definition capability.
    pub fn materialized_shape_dependencies(
        &self,
        current: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<(ConeIdentity, PersistentTypeId)>, HirDependencyTypeRelationError> {
        use HirDependencyTypeRelationError as Error;
        let path = WirePath::root().field(10);
        let mut result = Vec::new();
        for reference in self.records() {
            meter.charge_work(1, &path)?;
            for site in reference.type_sites().records() {
                meter.charge_work(1, &path)?;
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
                meter.charge_work(
                    2 * (1 + u64::from(identities.identity_count().max(1).ilog2())),
                    &path,
                )?;
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
                meter.check_table_entries(result.len() as u64 + 1, &path)?;
                meter.charge_owned_bytes(
                    std::mem::size_of::<(ConeIdentity, PersistentTypeId)>() as u64,
                    &path,
                )?;
                meter.try_reserve_collection_slots(&mut result, 1, &path)?;
                result.push((source.origin(), owner));
            }
        }
        meter.charge_work(
            (result.len() as u64).saturating_mul(1 + u64::from(result.len().max(1).ilog2())),
            &path,
        )?;
        result.sort_unstable();
        result.dedup();
        Ok(result)
    }
}
