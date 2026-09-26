//! Actual Link symbol uses replayed from the shared artifact and object closure.

use scoop_lir as lir;
use scoop_wire::{WireError, WirePath};

use crate::ReplayedLayoutLinkObjectContentsV1;
use crate::link_object::*;

mod code;
mod coverage;
mod errors;
mod finalization;
mod requirements;
mod verification;
pub use errors::LayoutLinkSymbolUseError;
pub(crate) use verification::{ReplayInputs, replay};

/// Complete symbols, final objects and the verified Code identity.
#[derive(Debug)]
pub struct ReplayedLayoutLinkSymbolUsesV1 {
    objects: ReplayedLayoutLinkObjectContentsV1,
    defined: CanonicalDefinedLinkSymbolOwnerSetV1,
    native: lir::CanonicalNativeExternalRequirementSurfaceV1,
    undefined: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    finalized: VerifiedCodeLinkObjectMemberSetV2,
    code: crate::CodeFingerprint,
    production: crate::SingleConeProductionCodeProjectionV1,
}

impl ReplayedLayoutLinkSymbolUsesV1 {
    pub const fn code_fingerprint(&self) -> crate::CodeFingerprint {
        self.code
    }

    pub const fn production_projection(&self) -> &crate::SingleConeProductionCodeProjectionV1 {
        &self.production
    }

    pub const fn provider(&self) -> scoop_identity::ConeIdentity {
        self.objects.provider()
    }

    pub const fn object_contents(&self) -> &ReplayedLayoutLinkObjectContentsV1 {
        &self.objects
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined
    }

    pub const fn native_requirements(&self) -> &lir::CanonicalNativeExternalRequirementSurfaceV1 {
        &self.native
    }

    pub const fn undefined_partitions(
        &self,
    ) -> &FinalizedLayoutUndefinedSymbolRequirementPartitionsV1 {
        &self.undefined
    }

    pub const fn final_objects(&self) -> &VerifiedEntryPatchSetV2 {
        self.finalized.final_objects()
    }

    pub const fn link_objects(&self) -> &VerifiedCodeLinkObjectMemberSetV2 {
        &self.finalized
    }
}
