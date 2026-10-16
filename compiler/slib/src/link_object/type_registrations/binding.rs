use crate::SlibMemberId;
use crate::link_object::{
    MissingDigestPatchSiteV1, VerifiedMaterializedPatchSiteV1, VerifiedScoopLirDigestPatchSiteSetV1,
};
use scoop_identity::PersistentExactTypeId;
use scoop_lir::{
    StrongDescriptorReference, StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1,
    StrongTypeRegistrationPlanSet,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRegistrationPatchV1 {
    exact_type: PersistentExactTypeId,
    descriptor_definition_patch: VerifiedMaterializedPatchSiteV1,
    layout_fingerprint_patch: VerifiedMaterializedPatchSiteV1,
}
impl TypeRegistrationPatchV1 {
    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }
    pub const fn member(&self) -> SlibMemberId {
        self.descriptor_definition_patch.member()
    }
    pub const fn descriptor_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.descriptor_definition_patch
    }
    pub const fn layout_fingerprint_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.layout_fingerprint_patch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRegistrationDataV1<
    D = StrongTypeDescriptorRefV1,
    C = StrongTypeDispatchCallableRefV1,
> {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanSet<D, C>,
    registrations: Vec<TypeRegistrationPatchV1>,
}
impl<D: StrongDescriptorReference, C: Clone> TypeRegistrationDataV1<D, C> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }
    pub const fn plan(&self) -> &StrongTypeRegistrationPlanSet<D, C> {
        &self.plan
    }
    pub fn registrations(&self) -> &[TypeRegistrationPatchV1] {
        &self.registrations
    }
}

pub fn bind_type_registrations_v1<D: StrongDescriptorReference, C: Clone>(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: StrongTypeRegistrationPlanSet<D, C>,
) -> Result<TypeRegistrationDataV1<D, C>, MissingDigestPatchSiteV1> {
    let registrations = plan
        .registrations()
        .iter()
        .map(|planned| {
            Ok(TypeRegistrationPatchV1 {
                exact_type: planned.exact_type(),
                descriptor_definition_patch: patch_sites
                    .require_site(planned.descriptor_definition_patch())?,
                layout_fingerprint_patch: patch_sites
                    .require_site(planned.layout_fingerprint_patch())?,
            })
        })
        .collect::<Result<_, MissingDigestPatchSiteV1>>()?;
    Ok(TypeRegistrationDataV1 {
        patch_sites,
        plan,
        registrations,
    })
}
