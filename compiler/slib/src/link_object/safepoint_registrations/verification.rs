//! Registration plans paired with the actual digest slots used during finalization.

use crate::SlibMemberId;
use crate::link_object::{
    DigestPatchSiteV1, MissingDigestPatchSiteV1, ScoopLirDigestPatchSitesV1, ScoopStackmapSetV1,
};
use scoop_identity::PersistentSafepointSiteId;
use scoop_lir::StrongSafepointRegistrationPlanSetV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafepointRegistrationPatchV1 {
    site: PersistentSafepointSiteId,
    normalized_stackmap_patch: DigestPatchSiteV1,
}
impl SafepointRegistrationPatchV1 {
    pub const fn site(&self) -> PersistentSafepointSiteId {
        self.site
    }
    pub const fn member(&self) -> SlibMemberId {
        self.normalized_stackmap_patch.member()
    }
    pub const fn normalized_stackmap_patch(&self) -> DigestPatchSiteV1 {
        self.normalized_stackmap_patch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafepointRegistrationDataV1 {
    stackmaps: ScoopStackmapSetV1,
    patch_sites: ScoopLirDigestPatchSitesV1,
    plan: StrongSafepointRegistrationPlanSetV1,
    registrations: Vec<SafepointRegistrationPatchV1>,
}
impl SafepointRegistrationDataV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn patch_sites(&self) -> &ScoopLirDigestPatchSitesV1 {
        &self.patch_sites
    }
    pub const fn plan(&self) -> &StrongSafepointRegistrationPlanSetV1 {
        &self.plan
    }
    pub fn registrations(&self) -> &[SafepointRegistrationPatchV1] {
        &self.registrations
    }
    pub const fn stackmaps(&self) -> &ScoopStackmapSetV1 {
        &self.stackmaps
    }
}

pub fn bind_safepoint_registrations_v1(
    stackmaps: ScoopStackmapSetV1,
    patch_sites: ScoopLirDigestPatchSitesV1,
    plan: StrongSafepointRegistrationPlanSetV1,
) -> Result<SafepointRegistrationDataV1, MissingDigestPatchSiteV1> {
    let registrations = plan
        .registrations()
        .iter()
        .map(|planned| {
            let normalized_stackmap_patch =
                patch_sites.require_site(planned.normalized_stackmap_patch())?;
            Ok(SafepointRegistrationPatchV1 {
                site: planned.site(),
                normalized_stackmap_patch,
            })
        })
        .collect::<Result<_, MissingDigestPatchSiteV1>>()?;
    Ok(SafepointRegistrationDataV1 {
        stackmaps,
        patch_sites,
        plan,
        registrations,
    })
}
