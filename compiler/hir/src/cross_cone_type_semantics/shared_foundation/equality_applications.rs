use super::*;
use scoop_identity::{GeneratedCallableKey, PersistentGeneratedCallableId};
use std::collections::BTreeMap;

impl SharedTypeMetadataV1<'_> {
    /// Queries original source applications, including unmaterialized defaults.
    /// This temporary identity index grants no machine definition or selection.
    pub fn derived_equality_applications(
        self,
    ) -> Result<BTreeMap<PersistentGeneratedCallableId, PersistentExactTypeId>, Error> {
        let mut applications = BTreeMap::new();
        for record in self
            .foundation
            .as_canonical()
            .type_source_generated_callable_records()
        {
            let GeneratedCallableKey::DerivedEquality { exact_owner } = record.key() else {
                continue;
            };

            applications.insert(record.id(), *exact_owner);
        }
        Ok(applications)
    }
}
