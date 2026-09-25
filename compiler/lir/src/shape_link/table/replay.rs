//! Candidate rows are contracts to compare, never evidence of actual use.

use super::*;
use crate::{ShapeLinkProviderV1, ShapeLinkSupportLookupV1, StrongObjectSymbolSurfaceV1};
use scoop_identity::{ConeIdentity, PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedCanonicalExternalShapeLinkImportsV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn replay<'a>(
        self,
        consumer: ConeIdentity,
        definitions: &StrongObjectSymbolSurfaceV1,
        providers: &[ShapeLinkProviderV1<'a>],
        support: &dyn ShapeLinkSupportLookupV1<'a>,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CanonicalExternalShapeLinkImportsV1<'a>, ShapeLinkError> {
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
            let expected = ExternalShapeLinkImportV1::replay(
                terminal,
                subject,
                consumer,
                definitions,
                support,
            )?;
            actual.validate_against(&expected)?;
            records.push(expected);
        }
        CanonicalExternalShapeLinkImportsV1::from_checked(records)
    }
}
