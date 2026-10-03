use scoop_identity::{
    ExactTypeKey, NominalDeclarationOwner, PersistentExactTypeId, SourceDeclarationKey,
};
use scoop_wire::WirePath;

use super::*;

impl TypeSiteRelations<'_> {
    pub(super) fn type_site_nominals(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<Vec<Nominal>, Error> {
        let path = WirePath::root();
        let owners = crate::collect_type_site_nominals(exact, |exact| {
            self.identities.canonical_key::<_, ExactTypeKey>(exact)
        })
        .map_err(|error| Error::Exact(Box::new(error)))?;
        let mut result = Vec::new();
        for owner in owners {
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

            scoop_wire::allocation::try_reserve(&mut result, 1, &path)?;
            result.push((provider, owner));
        }

        result.sort_unstable();
        Ok(result)
    }
}
