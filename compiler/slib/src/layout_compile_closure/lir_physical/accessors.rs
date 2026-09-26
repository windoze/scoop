use super::*;

impl PhysicalImportsReplayedCrossConeLayoutSections {
    pub fn identity(&self) -> ConeIdentity {
        self.semantic.metadata.identity()
    }
    pub fn coordinate(&self) -> &scoop_identity::ConeCoordinate {
        self.semantic.metadata.coordinate()
    }
    pub fn manifest(&self) -> &crate::BootstrapManifest {
        &self.semantic.metadata.manifest
    }
    pub fn identity_graph(&self) -> &scoop_identity::ValidatedIdentityGraph {
        &self.semantic.identities
    }
    pub fn hir_foundation(&self) -> &scoop_hir::OdrFreeHirFoundation {
        &self.semantic.foundations.hir
    }
    pub fn hir_production(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        &self.semantic.hir_core
    }
    pub fn hir_interface(&self) -> &scoop_hir::CrossConeHirInterfaceSectionV1 {
        &self.semantic.hir_interface
    }
    pub fn hir_type_semantics(&self) -> &scoop_hir::CrossConeTypeSemanticsSectionV1 {
        &self.semantic.hir_types
    }
    pub fn mir_foundation(&self) -> &scoop_mir::OdrFreeMirFoundation {
        &self.semantic.foundations.mir
    }
    pub fn mir_production(&self) -> &scoop_mir::CoreBootstrapBridgeSectionV1 {
        &self.semantic.mir_core
    }
    pub fn mir_cross_cone_bridge(&self) -> &scoop_mir::CrossConeMirBridgeSectionV1 {
        &self.semantic.mir_ordinary
    }
    pub fn mir_type_bridge(&self) -> &scoop_mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
        &self.mir
    }
    pub fn initialization_units(&self) -> &[mir::MirTypeBridgeInitializationUnitV1] {
        &self.units
    }
    pub fn lir_foundation(&self) -> &scoop_lir::OdrFreeLirFoundation {
        &self.semantic.foundations.lir
    }
    pub fn lir_cross_cone_bridge(&self) -> &scoop_lir::CrossConeLirBridgeSectionV1 {
        &self.ordinary
    }
    pub fn link_sections(&self) -> Option<&crate::DecodedCrossConeLayoutLinkOnlySections> {
        self.semantic.view.link()
    }
}
