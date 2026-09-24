use super::*;
use scoop_identity::{GeneratedCallableKey, PersistentGeneratedCallableId};
use scoop_wire::WirePath;
use std::collections::BTreeMap;

impl SharedTypeMetadataV1<'_> {
    /// Queries original source applications, including unmaterialized defaults.
    /// This temporary identity index grants no machine definition or selection.
    pub fn derived_equality_applications(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<BTreeMap<PersistentGeneratedCallableId, PersistentExactTypeId>, Error> {
        let mut applications = BTreeMap::new();
        for record in self
            .foundation
            .as_canonical()
            .type_source_generated_callable_records()
        {
            meter.charge_work(1, &WirePath::root())?;
            let GeneratedCallableKey::DerivedEquality { exact_owner } = record.key() else {
                continue;
            };
            meter.check_table_entries(applications.len() as u64 + 1, &WirePath::root())?;
            meter.charge_collection_slots(1, &WirePath::root())?;
            meter.charge_work(
                u64::from(applications.len().max(1).ilog2()) + 1,
                &WirePath::root(),
            )?;
            applications.insert(record.id(), *exact_owner);
        }
        Ok(applications)
    }
}
