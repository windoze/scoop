use crate::SlibMemberId;
use crate::link_object::{
    MissingDigestPatchSiteV1, VerifiedMaterializedPatchSiteV1, VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_identity::PersistentStaticStorageId;
use scoop_lir::StrongStaticStorageRegistrationPlanSetV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticStorageRegistrationPatchV1 {
    storage: PersistentStaticStorageId,
    scan_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
    layout_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
}
impl StaticStorageRegistrationPatchV1 {
    pub const fn storage(&self) -> PersistentStaticStorageId {
        self.storage
    }
    pub const fn member(&self) -> SlibMemberId {
        self.scan_fingerprint_patch.member()
    }
    pub const fn scan_fingerprint_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.scan_fingerprint_patch
    }
    pub const fn layout_fingerprint_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.layout_fingerprint_patch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticStorageRegistrationDataV1 {
    plan: StrongStaticStorageRegistrationPlanSetV1,
    registrations: Vec<StaticStorageRegistrationPatchV1>,
}
impl StaticStorageRegistrationDataV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn plan(&self) -> &StrongStaticStorageRegistrationPlanSetV1 {
        &self.plan
    }
    pub fn registrations(&self) -> &[StaticStorageRegistrationPatchV1] {
        &self.registrations
    }
}

pub fn bind_static_storage_registrations_v1(
    patch_sites: &VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongStaticStorageRegistrationPlanSetV1,
) -> Result<StaticStorageRegistrationDataV1, MissingDigestPatchSiteV1> {
    let registrations = plan
        .registrations()
        .iter()
        .map(|planned| {
            Ok(StaticStorageRegistrationPatchV1 {
                storage: planned.semantic().storage(),
                scan_fingerprint_patch: patch_sites
                    .require_site(planned.scan_fingerprint_patch())?,
                layout_fingerprint_patch: patch_sites
                    .require_site(planned.layout_fingerprint_patch())?,
            })
        })
        .collect::<Result<_, MissingDigestPatchSiteV1>>()?;
    Ok(StaticStorageRegistrationDataV1 {
        plan,
        registrations,
    })
}
