//! Actual Link symbol uses replayed from the shared artifact and object closure.

use scoop_lir as lir;
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::ReplayedLayoutLinkObjectContentsV1;
use crate::link_object::*;

mod coverage;
mod errors;
mod finalization;
mod requirements;
mod resources;
mod verification;
pub use errors::LayoutLinkSymbolUseError;
pub(crate) use verification::{ReplayInputs, replay};

/// Owned symbols, final objects and complete Link object projections. Code
/// identity and the complete source relations remain necessary for an artifact.
#[derive(Debug)]
pub struct ReplayedLayoutLinkSymbolUsesV1<'input> {
    objects: ReplayedLayoutLinkObjectContentsV1<'input>,
    defined: CanonicalDefinedLinkSymbolOwnerSetV1,
    native: lir::CanonicalNativeExternalRequirementSurfaceV1,
    undefined: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    finalized: VerifiedCodeLinkObjectMemberSetV2,
}

impl<'input> ReplayedLayoutLinkSymbolUsesV1<'input> {
    pub const fn provider(&self) -> scoop_identity::ConeIdentity {
        self.objects.provider()
    }

    pub const fn object_contents(&self) -> &ReplayedLayoutLinkObjectContentsV1<'input> {
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
