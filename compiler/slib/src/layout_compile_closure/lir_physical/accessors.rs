use super::*;

impl PhysicalImportsReplayedCrossConeLayoutSections {
    pub(crate) fn shared_metadata(&self) -> std::rc::Rc<crate::graph::ArtifactMetadata> {
        std::rc::Rc::clone(&self.semantic.metadata)
    }
    pub(crate) fn metadata(&self) -> &crate::graph::ArtifactMetadata {
        &self.semantic.metadata
    }
    pub(crate) fn shared_identity_graph(
        &self,
    ) -> std::rc::Rc<scoop_identity::ValidatedIdentityGraph> {
        std::rc::Rc::clone(&self.semantic.identities)
    }
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
    pub fn hir_foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.semantic.foundations.hir
    }
    pub(crate) fn shared_hir_foundation(&self) -> std::rc::Rc<scoop_hir::CanonicalHirFoundation> {
        std::rc::Rc::clone(&self.semantic.foundations.hir)
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
    pub fn type_alias_expansions(&self) -> &scoop_hir::CanonicalTypeAliasExpansionsV1 {
        &self.semantic.hir_aliases
    }
    pub fn mir_foundation(&self) -> &scoop_mir::CanonicalMirFoundation {
        &self.semantic.foundations.mir
    }
    pub(crate) fn shared_mir_foundation(&self) -> std::rc::Rc<scoop_mir::CanonicalMirFoundation> {
        std::rc::Rc::clone(&self.semantic.foundations.mir)
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
    pub fn lir_foundation(&self) -> &scoop_lir::ConeLirFoundation {
        &self.semantic.foundations.lir
    }
    pub fn lir_cross_cone_bridge(&self) -> &scoop_lir::CrossConeLirBridgeSectionV1 {
        &self.ordinary
    }
    pub fn link_sections(&self) -> Option<&crate::DecodedCrossConeLayoutLinkOnlySections> {
        self.semantic.view.link()
    }
}
