use super::*;
use std::collections::BTreeSet;

impl Fixture {
    pub(in crate::exact_shape_support::tests) fn identities(&self) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(self.foundation.producer())
            .unwrap();
        let mut exacts = BTreeSet::new();
        let mut nominals = BTreeSet::new();
        for layout in self.layouts.records() {
            pending
                .register_authority(layout.identity().layout())
                .unwrap();
            exacts.insert(layout.identity().exact());
            if let ExactTypeKey::Nominal(nominal) = layout.identity().exact_record().key() {
                nominals.insert(*nominal);
            }
        }
        for exact in exacts {
            pending.register_authority(exact).unwrap();
        }
        for nominal in nominals {
            pending.register_authority(nominal).unwrap();
        }
        for scan in self.foundation.scans() {
            pending.register_authority(scan.id()).unwrap();
        }
        for plan in self.foundation.definition_plans() {
            pending.register_authority(plan.id()).unwrap();
        }
        pending.finish().unwrap()
    }
}
