//! Safepoint plans and the actual normalized stackmap records.

use crate::link_object::ScoopStackmapSetV1;
use scoop_lir::StrongSafepointRegistrationPlanSetV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafepointRegistrationDataV1 {
    stackmaps: ScoopStackmapSetV1,
    plan: StrongSafepointRegistrationPlanSetV1,
}
impl SafepointRegistrationDataV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn plan(&self) -> &StrongSafepointRegistrationPlanSetV1 {
        &self.plan
    }
    pub const fn stackmaps(&self) -> &ScoopStackmapSetV1 {
        &self.stackmaps
    }
}

pub fn bind_safepoint_registrations_v1(
    stackmaps: ScoopStackmapSetV1,
    plan: StrongSafepointRegistrationPlanSetV1,
) -> SafepointRegistrationDataV1 {
    SafepointRegistrationDataV1 { stackmaps, plan }
}
