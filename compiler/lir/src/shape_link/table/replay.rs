//! Read saved import references and select their already loaded provider data.

use super::*;
use crate::ShapeLinkProviderV1;
use scoop_identity::{ConeIdentity, PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedCanonicalExternalShapeLinkImportsV1 {
    pub(crate) fn read<'a>(
        self,
        consumer: ConeIdentity,
        providers: &[ShapeLinkProviderV1<'a>],

        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CanonicalExternalShapeLinkImportsV1, ShapeLinkError> {
        let path = WirePath::root();

        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.records.len(), &path)?;
        let mut previous: Option<Vec<u8>> = None;
        for actual in self.records {
            let current = key(&actual.provider, &actual.subject)?;
            if previous.as_ref().is_some_and(|key| key >= &current) {
                return Err(ShapeLinkError::Order);
            }
            previous = Some(current);

            let provider = identities.resolve(actual.provider)?;
            let subject = actual.subject.resolve(identities)?;

            let terminal = providers
                .iter()
                .find(|view| view.provider() == provider)
                .ok_or(ShapeLinkError::MissingProvider(provider))?;
            if provider == consumer {
                return Err(ShapeLinkError::LocalImport);
            }
            records.push(actual.read(terminal, subject, identities)?);
        }
        Ok(CanonicalExternalShapeLinkImportsV1 { records })
    }
}
