//! Common manifest inputs borrowed from complete Strong production records.

use scoop_lir::{
    ConeImagePlanV1, ConeProductionSection, DigestFinalizationPlanV1, EntryProductionPlanV1,
    GeneratedBridgePlanSetV1, RegistrationIdentitySurfaceV1,
};

pub(crate) struct ProductionPlanInputs<'a> {
    pub image: &'a ConeImagePlanV1,
    pub entry: &'a EntryProductionPlanV1,
    pub digest: &'a DigestFinalizationPlanV1,
    pub generated: &'a GeneratedBridgePlanSetV1,
    pub identities: &'a RegistrationIdentitySurfaceV1,
}

impl<'a, D, C, I> From<&'a ConeProductionSection<D, C, I>> for ProductionPlanInputs<'a> {
    fn from(section: &'a ConeProductionSection<D, C, I>) -> Self {
        Self {
            image: section.image_plan(),
            entry: section.entry_plan(),
            digest: section.digest_finalization_plan(),
            generated: section.generated_bridge_plan(),
            identities: section.registration_production().identities(),
        }
    }
}
