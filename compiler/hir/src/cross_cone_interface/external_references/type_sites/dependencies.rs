//! Temporary type-dependency query over the shared materialized occurrences.

use crate::{
    CanonicalExternalHirReferencesV1, ExternalHirTargetV1, HirDependencyCallReasonV1,
    HirDependencyTypeRelationError,
};
use scoop_identity::{
    ConeIdentity, ExactTypeKey, NominalDeclarationOwner, PersistentExactTypeId,
    SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WirePath};

impl CanonicalExternalHirReferencesV1 {
    /// Returns source-owned exact dependencies after ordinary occurrence,
    /// origin and fanout validation. This query grants no machine capability;
    /// MIR still compares the complete result with its selected type targets.
    pub fn materialized_type_dependencies(
        &self,
        current: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<(ConeIdentity, PersistentExactTypeId)>, HirDependencyTypeRelationError> {
        use HirDependencyTypeRelationError as Error;
        let path = WirePath::root().field(10);
        let mut result = Vec::new();
        for reference in self.records() {
            meter.charge_work(1, &path)?;
            for call in reference.call_sites().records() {
                meter.charge_work(1, &path)?;
                if !matches!(call.reason(), HirDependencyCallReasonV1::CastFailure { .. }) {
                    continue;
                }
                meter.charge_work(
                    2 * (1 + u64::from(identities.identity_count().max(1).ilog2())),
                    &path,
                )?;
                let key = identities
                    .canonical_key::<_, ExactTypeKey>(call.result())
                    .map_err(|error| Error::Identity(Box::new(error)))?;
                let ExactTypeKey::Nominal(owner) = key.as_ref() else {
                    return Err(Error::CallResult(call.position()));
                };
                let source = identities
                    .canonical_key::<_, SourceDeclarationKey>(*owner)
                    .map_err(|error| Error::Identity(Box::new(error)))?;
                if source.origin() == current || source.origin() != reference.origin() {
                    return Err(Error::Target(reference.target()));
                }
                push(&mut result, (source.origin(), call.result()), meter, &path)?;
            }
            if reference.type_sites().is_empty() {
                continue;
            }
            let ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(owner)) =
                reference.target()
            else {
                return Err(Error::Target(reference.target()));
            };
            meter.charge_work(
                2 * (1 + u64::from(identities.identity_count().max(1).ilog2())),
                &path,
            )?;
            let source = identities
                .canonical_key::<_, SourceDeclarationKey>(owner)
                .map_err(|error| Error::Identity(Box::new(error)))?;
            if source.origin() == current || source.origin() != reference.origin() {
                return Err(Error::Target(reference.target()));
            }
            let key = ExactTypeKey::Nominal(owner);
            meter.charge_sha256(
                PersistentExactTypeId::hash_stream_length(&key).map_err(|_| Error::Encoding)?,
                &path,
            )?;
            let exact = PersistentExactTypeId::from_key(&key).map_err(|_| Error::Encoding)?;
            identities
                .canonical_key::<_, ExactTypeKey>(exact)
                .map_err(|error| Error::Identity(Box::new(error)))?;
            push(&mut result, (reference.origin(), exact), meter, &path)?;
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

fn push(
    result: &mut Vec<(ConeIdentity, PersistentExactTypeId)>,
    record: (ConeIdentity, PersistentExactTypeId),
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), HirDependencyTypeRelationError> {
    meter.check_table_entries(result.len() as u64 + 1, path)?;
    meter.charge_owned_bytes(
        std::mem::size_of::<(ConeIdentity, PersistentExactTypeId)>() as u64,
        path,
    )?;
    meter.try_reserve_collection_slots(result, 1, path)?;
    result.push(record);
    Ok(())
}
