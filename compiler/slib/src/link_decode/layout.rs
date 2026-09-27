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

use crate::{
    DecodedCrossConeLayoutLinkClosureSectionV1, DecodedCrossConeLinkClosureSectionV1,
    DecodedLinkIdentityClosureSectionV1, DecodedSingleConeProductionManifestV1,
    ValidatedGraphArtifact,
};

mod accessors;
mod code_input;
mod decode;
mod shared;

pub(crate) use code_input::layout_code_strong_input;
pub use decode::CrossConeLayoutLinkSectionDecodeError;
pub use shared::DecodedCrossConeLayoutLinkOnlySections;
pub(crate) use shared::DecodedLayoutView;

/// Canonically decoded payloads from one exact
/// `cross-cone-layout-strong/1` Link view.
///
/// The Compile-only payloads are retained because later Link validation must
/// independently rebuild their identity, production, bridge, and layout
/// joins. Decoding them here grants no Compile or Link semantic authority.
#[derive(Debug)]
pub struct DecodedCrossConeLayoutLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    production_manifest: DecodedSingleConeProductionManifestV1,
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
    cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
    layout_link_closure: DecodedCrossConeLayoutLinkClosureSectionV1,
}

#[cfg(test)]
mod tests;
