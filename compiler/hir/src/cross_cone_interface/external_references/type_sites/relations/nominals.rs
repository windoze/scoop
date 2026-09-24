use scoop_identity::{
    ExactTypeKey, NominalDeclarationOwner, PersistentExactTypeId, SourceDeclarationKey,
};
use scoop_wire::WirePath;

use super::*;

impl TypeSiteRelations<'_> {
    pub(super) fn type_site_nominals(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<Nominal>, Error> {
        let path = WirePath::root();
        let owners = crate::collect_type_site_nominals(
            exact,
            |exact| self.identities.canonical_key::<_, ExactTypeKey>(exact),
            meter,
        )
        .map_err(|error| Error::Exact(Box::new(error)))?;
        let mut result = Vec::new();
        for owner in owners {
            meter.charge_work(1 + self.dependencies.len() as u64, &path)?;
            let source = match owner {
                NominalDeclarationOwner::Concrete(owner) => self
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(owner),
                NominalDeclarationOwner::GenericTemplate(owner) => {
                    self.identities
                        .canonical_key::<_, SourceDeclarationKey>(owner)
                }
            }
            .map_err(|error| Error::Identity(Box::new(error)))?;
            let provider = source.origin();
            if provider == self.current {
                continue;
            }
            if !self.dependencies.contains(&provider) {
                return Err(Error::UnreachableNominal { provider, owner });
            }
            meter.charge_owned_bytes(std::mem::size_of::<Nominal>() as u64, &path)?;
            meter.try_reserve_collection_slots(&mut result, 1, &path)?;
            result.push((provider, owner));
        }
        let count = result.len() as u64;
        meter.charge_work(
            count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
            &path,
        )?;
        result.sort_unstable();
        Ok(result)
    }
}
