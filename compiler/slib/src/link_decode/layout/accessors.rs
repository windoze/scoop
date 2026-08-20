use super::*;
use crate::{ArtifactFingerprint, DependencyRecord};

impl DecodedCrossConeLayoutLinkSections<'_> {
    pub const fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> scoop_identity::ConeIdentity {
        self.graph.identity()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.graph.direct_dependencies()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_core_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface_wire(&self) -> &DecodedCrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn hir_type_semantics_wire(&self) -> &DecodedCrossConeTypeSemanticsSectionV1 {
        &self.hir_type_semantics
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn mir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeMirBridgeSectionV1 {
        &self.mir_cross_cone_bridge
    }

    pub const fn mir_type_bridge_wire(&self) -> &DecodedCrossConeMirTypeBridgeSectionV1 {
        &self.mir_type_bridge
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedConeProductionSectionV2 {
        &self.lir_strong_production
    }

    pub const fn lir_cross_cone_bridge_wire(&self) -> &DecodedCrossConeLirBridgeSectionV1 {
        &self.lir_cross_cone_bridge
    }

    pub const fn lir_layout_abi_wire(&self) -> &DecodedCrossConeLayoutAbiSectionV1 {
        &self.lir_layout_abi
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn cross_cone_link_closure_wire(&self) -> &DecodedCrossConeLinkClosureSectionV1 {
        &self.cross_cone_link_closure
    }

    pub const fn layout_link_closure_wire(&self) -> &DecodedCrossConeLayoutLinkClosureSectionV1 {
        &self.layout_link_closure
    }
}
