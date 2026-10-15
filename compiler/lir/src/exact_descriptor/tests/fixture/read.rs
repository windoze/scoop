use super::*;

impl Fixture {
    pub(in crate::exact_descriptor::tests) fn read_graph(&self) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_canonical_authority(self.exact.clone())
            .unwrap();
        for layout in self.foundation.layouts() {
            pending.register_authority(layout.id()).unwrap();
        }
        for plan in self.foundation.definition_plans() {
            pending.register_authority(plan.id()).unwrap();
        }
        pending.register_authority(self.vtable).unwrap();
        pending.finish().unwrap()
    }
    pub(in crate::exact_descriptor::tests) fn layouts(&self) -> &CanonicalExactLayoutExportsV1 {
        &self.layouts
    }
}
