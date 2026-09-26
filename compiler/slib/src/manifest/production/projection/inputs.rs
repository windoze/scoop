//! Common manifest inputs borrowed from complete Strong production records.

use scoop_lir::{
    ConeImagePlanV1, EntryProductionPlanV1, GeneratedBridgePlanSetV1,
    StrongDigestFinalizationPlanV1, StrongProductionSection, StrongRegistrationIdentitySurfaceV1,
};

pub(crate) struct ProductionPlanInputs<'a> {
    pub image: &'a ConeImagePlanV1,
    pub entry: &'a EntryProductionPlanV1,
    pub digest: &'a StrongDigestFinalizationPlanV1,
    pub generated: &'a GeneratedBridgePlanSetV1,
    pub identities: &'a StrongRegistrationIdentitySurfaceV1,
}

impl<'a, D, C, I> From<&'a StrongProductionSection<D, C, I>> for ProductionPlanInputs<'a> {
    fn from(section: &'a StrongProductionSection<D, C, I>) -> Self {
        Self {
            image: section.image_plan(),
            entry: section.entry_plan(),
            digest: section.digest_finalization_plan(),
            generated: section.generated_bridge_plan(),
            identities: section.registration_production().identities(),
        }
    }
}
