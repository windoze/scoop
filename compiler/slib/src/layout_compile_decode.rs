//! Exact Compile-view section decoding for the M23-6 layout profile.

use scoop_hir::{
    DecodedCoreBootstrapInterfaceSectionV1, DecodedCrossConeHirInterfaceSectionV1,
    DecodedCrossConeTypeSemanticsSectionV1, DecodedHirFoundation,
};
use scoop_lir::{
    DecodedCrossConeLayoutAbiSectionV1, DecodedCrossConeLirBridgeSectionV1, DecodedLirFoundation,
    DecodedStrongProductionSectionV2,
};
use scoop_mir::{
    DecodedCoreBootstrapBridgeSectionV1, DecodedCrossConeMirBridgeSectionV1,
    DecodedCrossConeMirTypeBridgeSectionV1, DecodedMirFoundation,
};

use crate::ValidatedGraphArtifact;

mod accessors;
mod decode;
pub use decode::CrossConeLayoutCompileSectionDecodeError;

/// Canonically decoded payloads from one exact
/// `cross-cone-layout-strong/1` Compile view.
///
/// These wire values have passed profile inventory and semantic-fingerprint
/// checks. They still carry no identity, source, selection, layout, or
/// production authority; later typed transitions must validate those joins.
#[derive(Debug)]
pub struct DecodedCrossConeLayoutCompileSections<'input> {
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
    lir_strong_production: DecodedStrongProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

#[cfg(test)]
mod tests;
