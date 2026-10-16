//! Registration plans paired with the actual digest slots used during finalization.

use crate::SlibMemberId;
use crate::link_object::{
    MissingDigestPatchSiteV1, VerifiedMaterializedPatchSiteV1, VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_identity::PersistentCallableBodyId;
use scoop_lir::StrongCallableRegistrationPlanSetV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableRegistrationPatchV1 {
    body: PersistentCallableBodyId,
    body_definition_patch: VerifiedMaterializedPatchSiteV1,
}
impl CallableRegistrationPatchV1 {
    pub const fn body(&self) -> PersistentCallableBodyId {
        self.body
    }
    pub const fn member(&self) -> SlibMemberId {
        self.body_definition_patch.member()
    }
    pub const fn body_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.body_definition_patch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableRegistrationDataV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongCallableRegistrationPlanSetV1,
    registrations: Vec<CallableRegistrationPatchV1>,
}
impl CallableRegistrationDataV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }
    pub const fn plan(&self) -> &StrongCallableRegistrationPlanSetV1 {
        &self.plan
    }
    pub fn registrations(&self) -> &[CallableRegistrationPatchV1] {
        &self.registrations
    }
}

pub fn bind_callable_registrations_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongCallableRegistrationPlanSetV1,
) -> Result<CallableRegistrationDataV1, MissingDigestPatchSiteV1> {
    let registrations = plan
        .registrations()
        .iter()
        .map(|planned| {
            let body_definition_patch =
                patch_sites.require_site(planned.body_definition_patch())?;
            Ok(CallableRegistrationPatchV1 {
                body: planned.body(),
                body_definition_patch,
            })
        })
        .collect::<Result<_, MissingDigestPatchSiteV1>>()?;
    Ok(CallableRegistrationDataV1 {
        patch_sites,
        plan,
        registrations,
    })
}
