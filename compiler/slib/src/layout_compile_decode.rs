//! Shared semantic section decoding for the M23-6 Compile and Link views.

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1, DecodedCoreBootstrapInterfaceSectionV1,
    DecodedCrossConeHirInterfaceSectionV1, DecodedCrossConeTypeSemanticsSectionV1,
    DecodedHirFoundation,
};
use scoop_identity::ValidatedIdentityGraph;
use scoop_lir::{
    DecodedConeProductionSectionV2, DecodedCrossConeLayoutAbiSectionV1,
    DecodedCrossConeLirBridgeSectionV1, DecodedLirFoundation,
};
use scoop_mir::{
    DecodedCoreBootstrapBridgeSectionV1, DecodedCrossConeMirBridgeSectionV1,
    DecodedCrossConeMirTypeBridgeSectionV1, DecodedMirFoundation,
};

use crate::link_decode::DecodedLayoutView;
use crate::{ValidatedGraphArtifact, strong_compile_decode::CanonicalFoundationSet};

mod accessors;
mod decode;
mod hir_resolution;
mod lir_semantic;
mod mir_semantic;
mod transitions;
pub use decode::CrossConeLayoutCompileSectionDecodeError;
pub use hir_resolution::CrossConeLayoutHirResolutionError;
pub(crate) use lir_semantic::DecodedCrossConeLayoutLirCandidates;
pub use mir_semantic::CrossConeLayoutMirFrontValidationError;
pub(crate) use mir_semantic::{
    LayoutSemanticSections, PreparedCrossConeLayoutMirSections, PreparedLayoutMirSemanticParts,
};

/// Canonically decoded payloads from one exact
/// `cross-cone-layout-strong/1` Compile or Link view.
///
/// These wire values have passed profile inventory and semantic-fingerprint
/// checks. They still carry no identity, source, selection, layout, or
/// production authority; later typed transitions must validate those joins.
#[derive(Debug)]
pub struct DecodedCrossConeLayoutCompileSections<'input> {
    pub(crate) graph: ValidatedGraphArtifact<'input>,
    pub(crate) view: DecodedLayoutView,
    pub(crate) hir_foundation: DecodedHirFoundation,
    pub(crate) hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    pub(crate) hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    pub(crate) hir_type_semantics: DecodedCrossConeTypeSemanticsSectionV1,
    pub(crate) mir_foundation: DecodedMirFoundation,
    pub(crate) mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    pub(crate) mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    pub(crate) mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    pub(crate) lir_foundation: DecodedLirFoundation,
    pub(crate) lir_strong_production: DecodedConeProductionSectionV2,
    pub(crate) lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    pub(crate) lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

/// M23-6 Compile payloads whose complete HIR-to-LIR foundation identity
/// delta passed one transaction. Foundation structure and every new section
/// reference remain unvalidated.
pub struct IdentityCheckedCrossConeLayoutCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    view: DecodedLayoutView,
    identities: ValidatedIdentityGraph,
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
}

/// One M23-6 provider whose three foundation layers are structurally valid
/// and ODR-free. All production and cross-Cone semantic payloads remain
/// decoded references without lookup, selection, or machine-use authority.
pub struct FoundationValidatedCrossConeLayoutCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    view: DecodedLayoutView,
    identities: ValidatedIdentityGraph,
    foundations: CanonicalFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    hir_type_semantics: DecodedCrossConeTypeSemanticsSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_strong_production: DecodedConeProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

/// Foundation-validated M23-6 payloads whose old and new HIR transports were
/// resolved through the same typed identity graph. Their cross-table and
/// dependency semantics remain separate obligations.
pub struct ResolvedCrossConeLayoutHirSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    view: DecodedLayoutView,
    identities: ValidatedIdentityGraph,
    foundations: CanonicalFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    hir_type_semantics: CrossConeTypeSemanticsSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_strong_production: DecodedConeProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

/// Resolved M23-6 HIR payloads whose unchanged core-bootstrap production
/// contract was replayed against the same ODR-free foundation. General public
/// and type-semantics tables still require their complete semantic authorities.
pub struct HirProductionValidatedCrossConeLayoutSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    view: DecodedLayoutView,
    identities: ValidatedIdentityGraph,
    foundations: CanonicalFoundationSet,
    hir_core_production: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    hir_type_semantics: CrossConeTypeSemanticsSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_strong_production: DecodedConeProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

#[cfg(test)]
pub(crate) mod tests;
