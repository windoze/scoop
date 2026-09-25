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
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalExternalShapeLinkImportsV1<'a>, ShapeLinkError> {
        let path = WirePath::root();
        meter.check_table_entries(self.records.len() as u64, &path)?;
        meter.charge_nodes(self.records.len() as u64, &path)?;
        let mut records = Vec::new();
        meter.try_reserve_collection_slots(&mut records, self.records.len(), &path)?;
        let mut previous: Option<Vec<u8>> = None;
        for actual in self.records {
            let current = key(&actual.provider, &actual.subject, meter)?;
            if previous.as_ref().is_some_and(|key| key >= &current) {
                return Err(ShapeLinkError::Order);
            }
            previous = Some(current);
            meter.charge_work(identities.identity_count() as u64, &path)?;
            let provider = identities.resolve(actual.provider)?;
            let subject = actual.subject.resolve(identities, meter)?;
            meter.charge_work(providers.len() as u64, &path)?;
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
                meter,
            )?;
            actual.validate_against(&expected, meter)?;
            records.push(expected);
        }
        CanonicalExternalShapeLinkImportsV1::from_checked(records, meter)
    }
}
