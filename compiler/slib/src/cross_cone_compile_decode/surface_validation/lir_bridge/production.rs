//! Complete imported semantic data from the shared layout reader.

use crate::PhysicalImportsReplayedCrossConeLayoutSections;
use std::rc::Rc;

pub struct ValidatedCrossConeSemanticsProduction {
    artifact: Rc<PhysicalImportsReplayedCrossConeLayoutSections>,
}

impl ValidatedCrossConeSemanticsProduction {
    pub(crate) fn new(artifact: Rc<PhysicalImportsReplayedCrossConeLayoutSections>) -> Self {
        Self { artifact }
    }

    pub fn layout(&self) -> &PhysicalImportsReplayedCrossConeLayoutSections {
        &self.artifact
    }
    pub fn hir_core(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        self.artifact.hir_production()
    }
    pub fn hir_interface(&self) -> &scoop_hir::CrossConeHirInterfaceSectionV1 {
        self.artifact.hir_interface()
    }
    pub fn mir_core(&self) -> &scoop_mir::CoreBootstrapBridgeSectionV1 {
        self.artifact.mir_production()
    }
    pub fn mir_cross_cone(&self) -> &scoop_mir::CrossConeMirBridgeSectionV1 {
        self.artifact.mir_cross_cone_bridge()
    }
    pub fn lir_strong(&self) -> &scoop_lir::ConeProductionSectionV2 {
        self.artifact.lir_strong_production()
    }
    pub fn lir_cross_cone(&self) -> &scoop_lir::CrossConeLirBridgeSectionV1 {
        self.artifact.lir_cross_cone_bridge()
    }
    pub fn type_alias_expansions(&self) -> &scoop_hir::CanonicalTypeAliasExpansionsV1 {
        self.artifact.type_alias_expansions()
    }
}
