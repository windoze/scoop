use super::*;
use scoop_identity::PersistentId;
use std::sync::Arc;

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub(super) fn nominal_provider(&self, owner: SourceNominalId) -> Result<ConeIdentity, Error> {
        match owner {
            SourceNominalId::Concrete(id) => self.source_provider(id),
            SourceNominalId::GenericTemplate(id) => self.source_provider(id),
        }
    }

    pub(super) fn source_provider<I: PersistentId + 'static>(
        &self,
        id: I,
    ) -> Result<ConeIdentity, Error> {
        let key = self.identity_key::<_, SourceDeclarationKey>(id)?;

        Ok(key.origin())
    }

    pub(super) fn identity_key<I: PersistentId + 'static, K: Send + Sync + 'static>(
        &self,
        id: I,
    ) -> Result<Arc<K>, Error> {
        let identities = self.current.foundation.identities;

        identities
            .canonical_key(id)
            .map_err(|error| Error::Identity(error.to_string()))
    }
}
