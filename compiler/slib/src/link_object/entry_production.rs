//! Library or executable entry data used by digest patching and publication.

use super::{
    MissingDigestPatchSiteV1, VerifiedMaterializedPatchSiteV1, VerifiedScoopLirDigestPatchSiteSetV1,
};
use crate::SlibMemberId;
use scoop_lir::{EntryProductionPlanV1, ExecutableEntryPlanV1};

mod finalization;
pub use finalization::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableEntryDataV1 {
    plan: ExecutableEntryPlanV1,
    source_signature_patch: VerifiedMaterializedPatchSiteV1,
    gateway_definition_patch: VerifiedMaterializedPatchSiteV1,
}
impl ExecutableEntryDataV1 {
    pub fn plan(&self) -> &ExecutableEntryPlanV1 {
        &self.plan
    }
    pub const fn member(&self) -> SlibMemberId {
        self.source_signature_patch.member()
    }
    pub const fn source_signature_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.source_signature_patch
    }
    pub const fn gateway_definition_patch(&self) -> VerifiedMaterializedPatchSiteV1 {
        self.gateway_definition_patch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryProductionBranchV1 {
    Library,
    Executable(Box<ExecutableEntryDataV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryProductionDataV1 {
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    branch: EntryProductionBranchV1,
}
impl EntryProductionDataV1 {
    pub const fn patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.patch_sites
    }
    pub const fn branch(&self) -> &EntryProductionBranchV1 {
        &self.branch
    }
}

pub fn bind_entry_production_v1(
    patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    plan: EntryProductionPlanV1,
) -> Result<EntryProductionDataV1, MissingDigestPatchSiteV1> {
    let branch = match plan {
        EntryProductionPlanV1::Library => EntryProductionBranchV1::Library,
        EntryProductionPlanV1::Executable(plan) => {
            let source_signature_patch = patch_sites.require_site(plan.source_signature_patch())?;
            let gateway_definition_patch =
                patch_sites.require_site(plan.gateway_definition_patch())?;
            EntryProductionBranchV1::Executable(Box::new(ExecutableEntryDataV1 {
                plan: *plan,
                source_signature_patch,
                gateway_definition_patch,
            }))
        }
    };
    Ok(EntryProductionDataV1 {
        patch_sites,
        branch,
    })
}
