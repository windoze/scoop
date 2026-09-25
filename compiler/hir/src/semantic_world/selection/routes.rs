use crate::{
    DependencyBindingWitnessV1, DirectImportedTargetBinding, ExternalHirTargetV1,
    ImportedSemanticWorld,
};
use scoop_identity::CallableTemplateOrigin;
use std::collections::BTreeMap;

impl ImportedSemanticWorld<'_> {
    pub(super) fn direct_binding_witnesses(
        &self,
        callables: &BTreeMap<CallableTemplateOrigin, DirectImportedTargetBinding>,
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
        // A property accessor retains the source binding of its property.
        // This witnesses default-template references without adding call sites.
        for (callable, binding) in callables {
            let routes = witnesses
                .entry(ExternalHirTargetV1::Callable(*callable))
                .or_default();
            routes.extend(
                binding
                    .sources()
                    .map(|source| source.witness().dependency().clone()),
            );
            routes.sort_unstable();
            routes.dedup();
        }
        witnesses
    }
}
