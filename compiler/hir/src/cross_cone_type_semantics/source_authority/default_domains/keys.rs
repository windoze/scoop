use super::*;
use scoop_identity::PersistentId;
use std::sync::Arc;

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub(super) fn nominal_provider(
        &self,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<ConeIdentity, Error> {
        match owner {
            SourceNominalId::Concrete(id) => self.source_provider(id, meter, path),
            SourceNominalId::GenericTemplate(id) => self.source_provider(id, meter, path),
        }
    }

    pub(super) fn source_provider<I: PersistentId + 'static>(
        &self,
        id: I,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<ConeIdentity, Error> {
        let key = self.identity_key::<_, SourceDeclarationKey>(id, meter, path)?;
        NominalRepresentationSupportV1::charge_source_key_resources(&key, meter, path)?;
        Ok(key.origin())
    }

    pub(super) fn identity_key<I: PersistentId + 'static, K: Send + Sync + 'static>(
        &self,
        id: I,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Arc<K>, Error> {
        let identities = self.current.foundation.identities;
        meter.charge_edges(1, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(
            (u64::from(identities.identity_count().max(1).ilog2()) + 1) * 65,
            path,
        )?;
        identities
            .canonical_key(id)
            .map_err(|error| Error::Identity(error.to_string()))
    }
}
