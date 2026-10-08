use super::*;
use scoop_identity::{GeneratedCallableKey, PersistentGeneratedCallableId};
use std::collections::BTreeMap;

impl SharedTypeMetadataV1<'_> {
    pub(crate) fn derived_equality_access(
        self,
        owner: crate::SourceNominalId,
    ) -> Result<crate::DeclarationAccessSourceV1, Error> {
        let declaration = self
            .public
            .nominal_interfaces()
            .declaration(owner)
            .ok_or(Error::InheritanceSource(owner))?;
        let key = match owner {
            crate::SourceNominalId::Concrete(id) => {
                self.identities
                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)?
            }
            crate::SourceNominalId::GenericTemplate(id) => {
                self.identities
                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)?
            }
        };
        let access = sources::declaration_access(self, declaration, &key)?;
        crate::DeclarationAccessSourceV1::try_new(
            crate::DeclaredVisibilityV1::Public,
            access.lexical_owners().to_vec(),
            access.definition_origin().clone(),
        )
        .map_err(|_| Error::InheritanceSource(owner))
    }

    /// Queries original source applications, including unmaterialized defaults.
    /// This temporary identity index grants no machine definition or selection.
    pub fn derived_equality_applications(
        self,
    ) -> Result<BTreeMap<PersistentGeneratedCallableId, PersistentExactTypeId>, Error> {
        let mut applications = BTreeMap::new();
        for record in self.foundation.type_source_generated_callable_records() {
            let GeneratedCallableKey::DerivedEquality { exact_owner } = record.key() else {
                continue;
            };

            applications.insert(record.id(), *exact_owner);
        }
        Ok(applications)
    }
}
