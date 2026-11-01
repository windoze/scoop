//! Atomic Link-view decoding for the M23-6 layout profile.

use scoop_hir::{
    DecodedCoreBootstrapInterfaceSectionV1, DecodedCrossConeHirInterfaceSectionV1,
    DecodedCrossConeTypeSemanticsSectionV1, DecodedHirFoundation,
};
use scoop_lir::{
    DecodedConeProductionSectionV2, DecodedCrossConeLayoutAbiSectionV1,
    DecodedCrossConeLirBridgeSectionV1, DecodedLirFoundation,
};
use scoop_mir::{
    DecodedCoreBootstrapBridgeSectionV1, DecodedCrossConeMirBridgeSectionV1,
    DecodedCrossConeMirTypeBridgeSectionV1, DecodedMirFoundation,
};

use crate::{DecodedLinkIdentityClosureSectionV1, ValidatedGraphArtifact};

mod accessors;
mod decode;
mod machine;
mod shared;
pub(crate) use machine::DecodedMachineLinkSections;

pub use decode::CrossConeLayoutLinkSectionDecodeError;
pub use shared::DecodedCrossConeLayoutLinkOnlySections;
pub(crate) use shared::DecodedLayoutView;

/// Decoded payloads from the current cross-cone generic Link view.
///
/// Compile sections needed for Link relations are retained alongside physical
/// data. Subsequent validation joins their typed identities, signatures, and
/// layouts to object definitions.
#[derive(Debug)]
pub struct DecodedCrossConeLayoutLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    hir_type_semantics: DecodedCrossConeTypeSemanticsSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_strong_production: DecodedConeProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,

    link_support: crate::DecodedLirLinkSupportSectionV1,
}

#[cfg(test)]
mod tests;
