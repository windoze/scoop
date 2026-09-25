//! Actual Link symbol uses replayed from the shared artifact and object closure.

use scoop_lir as lir;
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::ReplayedLayoutLinkObjectContentsV1;
use crate::link_object::*;

mod errors;
mod requirements;
mod resources;
mod verification;
pub use errors::LayoutLinkSymbolUseError;
pub(crate) use verification::{ReplayInputs, replay};

/// Owned object and symbol evidence. Final coverage, dependency fingerprints
/// and Code identity remain necessary before this can become a Link artifact.
#[derive(Debug)]
pub struct ReplayedLayoutLinkSymbolUsesV1<'input> {
    objects: ReplayedLayoutLinkObjectContentsV1<'input>,
    defined: CanonicalDefinedLinkSymbolOwnerSetV1,
    native: lir::CanonicalNativeExternalRequirementSurfaceV1,
    undefined: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
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
}
