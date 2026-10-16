use crate::link_object::{
    MissingDigestPatchSiteV1, VerifiedMaterializedPatchSiteV1, VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_identity::PersistentInitializationUnitId;
use scoop_lir::StrongInitializationUnitRegistrationPlanSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializationRegistrationPatchV1 {
    unit: PersistentInitializationUnitId,
    gateway_definition_patch: Option<VerifiedMaterializedPatchSiteV1>,
}
impl InitializationRegistrationPatchV1 {
    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
    }
    pub const fn gateway_definition_patch(&self) -> Option<VerifiedMaterializedPatchSiteV1> {
        self.gateway_definition_patch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializationRegistrationDataV1<D = PersistentInitializationUnitId> {
    plan: StrongInitializationUnitRegistrationPlanSet<D>,
    registrations: Vec<InitializationRegistrationPatchV1>,
}
impl<D> InitializationRegistrationDataV1<D> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn plan(&self) -> &StrongInitializationUnitRegistrationPlanSet<D> {
        &self.plan
    }
    pub fn registrations(&self) -> &[InitializationRegistrationPatchV1] {
        &self.registrations
    }
}

pub fn bind_initialization_registrations_v1<D>(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongInitializationUnitRegistrationPlanSet<D>,
) -> Result<InitializationRegistrationDataV1<D>, MissingDigestPatchSiteV1> {
    let registrations = plan
        .registrations()
        .iter()
        .map(|planned| {
            Ok(InitializationRegistrationPatchV1 {
                unit: planned.semantic().unit(),
                gateway_definition_patch: planned
                    .schedule()
                    .gateway_definition_patch()
                    .map(|intent| patch_sites.require_site(intent))
                    .transpose()?,
            })
        })
        .collect::<Result<_, MissingDigestPatchSiteV1>>()?;
    Ok(InitializationRegistrationDataV1 {
        plan,
        registrations,
    })
}
