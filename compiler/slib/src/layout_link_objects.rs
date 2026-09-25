//! Physical Link replay retains the original final bytes and all object proofs.

use scoop_lir as lir;
use scoop_wire::BudgetMeter;

use crate::link_object::*;

mod errors;
mod registrations;
mod resources;
mod verification;
pub use errors::LayoutLinkObjectContentsError;
pub(crate) use verification::replay;

/// Complete object-content checks for one artifact. Undefined-use partitions,
/// dependency fingerprints and final Code identity still require replay.
#[derive(Debug)]
pub struct ReplayedLayoutLinkObjectContentsV1<'input> {
    objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    generated: Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    safepoints: VerifiedStrongSafepointRegistrationSetV1,
    callables: VerifiedStrongCallableRegistrationSetV1,
    types: VerifiedStrongTypeRegistrationSetV2,
    immortals: VerifiedStrongImmortalObjectRegistrationSetV1,
    storages: VerifiedStrongStaticStorageRegistrationSetV1,
    initializations: VerifiedStrongInitializationRegistrationSetV2,
    costs: resources::ReplayCosts,
}

impl<'input> ReplayedLayoutLinkObjectContentsV1<'input> {
    pub(crate) fn charge_strong_closure_copy(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<(), scoop_wire::WireError> {
        self.costs
            .copy_builtins(self.patch_sites().builtins(), meter)
    }

    pub(crate) fn charge_patch_site_copy(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<(), scoop_wire::WireError> {
        self.charge_strong_closure_copy(meter)?;
        resources::copy_plan(self.patch_sites().digest_plan(), meter)?;
        resources::slots::<VerifiedMaterializedPatchSiteV1>(
            self.patch_sites().sites().len() as u64,
            meter,
        )
    }

    pub const fn provider(&self) -> scoop_identity::ConeIdentity {
        self.safepoints.producer()
    }

    pub const fn objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.objects
    }

    pub fn generated_objects(&self) -> &[GeneratedCBridgeObjectCandidateV1<'input>] {
        &self.generated
    }

    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        self.safepoints.patch_sites()
    }

    pub const fn stackmaps(&self) -> &VerifiedScoopLirStackmapSetV1 {
        self.safepoints.stackmaps()
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointRegistrationSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &VerifiedStrongTypeRegistrationSetV2 {
        &self.types
    }

    pub const fn immortals(&self) -> &VerifiedStrongImmortalObjectRegistrationSetV1 {
        &self.immortals
    }

    pub const fn storages(&self) -> &VerifiedStrongStaticStorageRegistrationSetV1 {
        &self.storages
    }

    pub const fn initializations(&self) -> &VerifiedStrongInitializationRegistrationSetV2 {
        &self.initializations
    }
}
