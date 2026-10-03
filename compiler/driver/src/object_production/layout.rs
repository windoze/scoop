//! Actual V2 object verification and finalization for layout-profile archives.

use super::*;
use scoop_codegen::EmittedConeObjectSetV2;
use scoop_lir::ConeProductionSectionV2;
use scoop_slib as slib;

mod finalization;
mod registrations;
mod requirements;
mod verification;

pub(crate) use requirements::complete_requirements;
pub(crate) use verification::prepare;

pub(crate) struct PreparedLayoutObjects {
    pub(crate) target_selection: ValidatedLirTargetSelection,
    pub(crate) foundation: ConeLirFoundation,
    pub(crate) production: ConeProductionSectionV2,
    pub(crate) c_bridge_profile: CBridgeToolchainProfileV1,
    pub(crate) patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
    bindings: PlannedObjectBindings,
}

impl PreparedLayoutObjects {
    fn candidates(&self) -> Vec<ScoopLirObjectCandidateV1<'_>> {
        self.bindings
            .scoop_lir_members
            .iter()
            .map(|member| ScoopLirObjectCandidateV1::new(member.plan.member_id(), &member.bytes))
            .collect()
    }
}
