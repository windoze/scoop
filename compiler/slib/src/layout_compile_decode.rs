//! Exact Compile-view section decoding for the M23-6 layout profile.

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1, DecodedCoreBootstrapInterfaceSectionV1,
    DecodedCrossConeHirInterfaceSectionV1, DecodedCrossConeTypeSemanticsSectionV1,
    DecodedHirFoundation,
};
use scoop_identity::ValidatedIdentityGraph;
use scoop_lir::{
    DecodedCrossConeLayoutAbiSectionV1, DecodedCrossConeLirBridgeSectionV1, DecodedLirFoundation,
    DecodedStrongProductionSectionV2,
};
use scoop_mir::{
    DecodedCoreBootstrapBridgeSectionV1, DecodedCrossConeMirBridgeSectionV1,
    DecodedCrossConeMirTypeBridgeSectionV1, DecodedMirFoundation,
};

use crate::{ValidatedGraphArtifact, strong_compile_decode::OdrFreeStrongFoundationSet};

mod accessors;
mod decode;
mod hir_resolution;
mod mir_semantic;
mod transitions;
pub use decode::CrossConeLayoutCompileSectionDecodeError;
pub use hir_resolution::CrossConeLayoutHirResolutionError;
pub use mir_semantic::CrossConeLayoutMirFrontValidationError;
pub(crate) use mir_semantic::{PreparedCrossConeLayoutMirSections, PreparedLayoutMirSemanticParts};

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

/// M23-6 Compile payloads whose complete HIR-to-LIR foundation identity
/// delta passed one transaction. Foundation structure and every new section
/// reference remain unvalidated.
pub struct IdentityCheckedCrossConeLayoutCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
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
    lir_strong_production: DecodedStrongProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

/// One M23-6 provider whose three foundation layers are structurally valid
/// and ODR-free. All production and cross-Cone semantic payloads remain
/// decoded references without lookup, selection, or machine-use authority.
pub struct FoundationValidatedCrossConeLayoutCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    hir_type_semantics: DecodedCrossConeTypeSemanticsSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_strong_production: DecodedStrongProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

/// Foundation-validated M23-6 payloads whose old and new HIR transports were
/// resolved through the same typed identity graph. Their cross-table and
/// dependency semantics remain separate obligations.
pub struct ResolvedCrossConeLayoutHirSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    hir_type_semantics: CrossConeTypeSemanticsSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_strong_production: DecodedStrongProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

/// Resolved M23-6 HIR payloads whose unchanged core-bootstrap production
/// contract was replayed against the same ODR-free foundation. General public
/// and type-semantics tables still require their complete semantic authorities.
pub struct HirProductionValidatedCrossConeLayoutSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    hir_type_semantics: CrossConeTypeSemanticsSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    mir_cross_cone_bridge: DecodedCrossConeMirBridgeSectionV1,
    mir_type_bridge: DecodedCrossConeMirTypeBridgeSectionV1,
    lir_strong_production: DecodedStrongProductionSectionV2,
    lir_cross_cone_bridge: DecodedCrossConeLirBridgeSectionV1,
    lir_layout_abi: DecodedCrossConeLayoutAbiSectionV1,
}

#[cfg(test)]
pub(crate) mod tests;
