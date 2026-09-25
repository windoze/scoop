//! Temporary type-dependency query over the shared materialized occurrences.

use crate::{
    CanonicalExternalHirReferencesV1, ExternalHirTargetV1, HirDependencyCallReasonV1,
    HirDependencyTypeRelationError,
};
use scoop_identity::{
    ConeIdentity, ExactTypeKey, NominalDeclarationOwner, PersistentExactTypeId,
    SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_wire::WirePath;

impl CanonicalExternalHirReferencesV1 {
    /// Returns source-owned exact dependencies after ordinary occurrence,
    /// origin and fanout validation. This query grants no machine capability;
    /// MIR still compares the complete result with its selected type targets.
    pub fn materialized_type_dependencies(
        &self,
        current: ConeIdentity,
        identities: &ValidatedIdentityGraph,
    ) -> Result<Vec<(ConeIdentity, PersistentExactTypeId)>, HirDependencyTypeRelationError> {
        use HirDependencyTypeRelationError as Error;
        let path = WirePath::root().field(10);
        let mut result = Vec::new();
        for reference in self.records() {
            for call in reference.call_sites().records() {
                if !matches!(call.reason(), HirDependencyCallReasonV1::CastFailure { .. }) {
                    continue;
                }

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
                push(&mut result, (source.origin(), call.result()), &path)?;
            }
            if reference.type_sites().is_empty() {
                continue;
            }
            let ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(owner)) =
                reference.target()
            else {
                return Err(Error::Target(reference.target()));
            };

            let source = identities
                .canonical_key::<_, SourceDeclarationKey>(owner)
                .map_err(|error| Error::Identity(Box::new(error)))?;
            if source.origin() == current || source.origin() != reference.origin() {
                return Err(Error::Target(reference.target()));
            }
            let key = ExactTypeKey::Nominal(owner);

            let exact = PersistentExactTypeId::from_key(&key).map_err(|_| Error::Encoding)?;
            identities
                .canonical_key::<_, ExactTypeKey>(exact)
                .map_err(|error| Error::Identity(Box::new(error)))?;
            push(&mut result, (reference.origin(), exact), &path)?;
        }

        result.sort_unstable();
        result.dedup();
        Ok(result)
    }
}

fn push(
    result: &mut Vec<(ConeIdentity, PersistentExactTypeId)>,
    record: (ConeIdentity, PersistentExactTypeId),

    path: &WirePath,
) -> Result<(), HirDependencyTypeRelationError> {
    scoop_wire::allocation::try_reserve(result, 1, path)?;
    result.push(record);
    Ok(())
}
