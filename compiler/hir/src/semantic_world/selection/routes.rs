use crate::{DependencyBindingWitnessV1, ExternalHirTargetV1, ImportedSemanticWorld};
use std::collections::BTreeMap;

impl ImportedSemanticWorld<'_> {
    pub(super) fn direct_binding_witnesses(
        &self,
    ) -> BTreeMap<ExternalHirTargetV1, Vec<DependencyBindingWitnessV1>> {
        let mut witnesses: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for provider in self.direct_providers() {
            for binding in provider.public_bindings() {
                let target = ExternalHirTargetV1::from(binding.target().persistent());
                let routes = witnesses.entry(target).or_default();
                routes.extend(
                    binding
                        .lookup_sources()
                        .iter()
                        .map(|source| source.witness().dependency().clone()),
                );
                routes.sort_unstable();
                routes.dedup();
            }
        }
        witnesses
    }
}
