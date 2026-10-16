//! Common manifest inputs borrowed from complete Strong production records.

use scoop_lir::{ConeProductionSection, RegistrationIdentitySurfaceV1};

pub(crate) struct ProductionPlanInputs<'a> {
    pub identities: &'a RegistrationIdentitySurfaceV1,
}

impl<'a, D, C, I> From<&'a ConeProductionSection<D, C, I>> for ProductionPlanInputs<'a> {
    fn from(section: &'a ConeProductionSection<D, C, I>) -> Self {
        Self {
            identities: section.registration_production().identities(),
        }
    }
}
