//! Physical Link replay retains the original final bytes and all object proofs.

use scoop_lir as lir;

use crate::link_object::*;

mod errors;
mod registrations;
mod verification;
pub use errors::LayoutLinkObjectContentsError;
pub(crate) use verification::replay;

/// Complete object-content checks for one artifact. Undefined-use partitions,
/// dependency fingerprints and final Code identity still require replay.
#[derive(Debug)]
pub struct ReplayedLayoutLinkObjectContentsV1 {
    objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    generated: Vec<(crate::SlibMemberId, Vec<u8>)>,
    safepoints: VerifiedStrongSafepointRegistrationSetV1,
    callables: VerifiedStrongCallableRegistrationSetV1,
    types: VerifiedStrongTypeRegistrationSetV2,
    immortals: VerifiedStrongImmortalObjectRegistrationSetV1,
    storages: VerifiedStrongStaticStorageRegistrationSetV1,
    initializations: VerifiedStrongInitializationRegistrationSetV2,
}

impl ReplayedLayoutLinkObjectContentsV1 {
    pub const fn provider(&self) -> scoop_identity::ConeIdentity {
        self.safepoints.producer()
    }

    pub const fn objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.objects
    }

    pub fn generated_objects(
        &self,
    ) -> impl ExactSizeIterator<Item = GeneratedCBridgeObjectCandidateV1<'_>> {
        self.generated
            .iter()
            .map(|(member, bytes)| GeneratedCBridgeObjectCandidateV1::new(*member, bytes))
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
