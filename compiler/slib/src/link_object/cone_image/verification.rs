//! Bind the runtime image plan to its generated digest slot.

use crate::SlibMemberId;
use crate::link_object::{
    MissingDigestPatchSiteV1, VerifiedMaterializedPatchSiteV1, VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_lir::ConeImagePlanV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeImageDataV1 {
    plan: ConeImagePlanV1,
    image_patch: VerifiedMaterializedPatchSiteV1,
}
impl ConeImageDataV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.cone().identity()
    }
    pub const fn plan(&self) -> &ConeImagePlanV1 {
        &self.plan
    }
    pub const fn member(&self) -> SlibMemberId {
        self.image_patch.member()
    }
    pub const fn image_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.image_patch
    }
}

pub fn bind_cone_image_v1(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: ConeImagePlanV1,
) -> Result<ConeImageDataV1, MissingDigestPatchSiteV1> {
    let image_patch = patch_sites.require_site(plan.fingerprint_patch())?;
    Ok(ConeImageDataV1 { plan, image_patch })
}
