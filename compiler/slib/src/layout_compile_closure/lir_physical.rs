//! Complete physical imports retained with their dependency sections.

use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;
use std::collections::BTreeMap;

use super::{
    LirDependencyGraphReplayedCrossConeLayoutClosure,
    LirDependencyGraphReplayedCrossConeLayoutSections,
    lir_constituents::LirConstituentsValidatedCrossConeLayoutSections,
};
use crate::dependency_reachability::transitive_positions;

mod accessors;
mod driver;
mod errors;
mod objects;
mod replay;
mod symbols;
pub use errors::{CrossConeLayoutLirPhysicalError, SharedLirPhysicalError};
pub use objects::LinkObjectsReplayedCrossConeLayoutClosure;
pub use symbols::LinkSymbolsReplayedCrossConeLayoutClosure;

pub struct PhysicalImportsReplayedCrossConeLayoutSections {
    semantic: crate::layout_compile_decode::LayoutSemanticSections,
    mir: mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    units: Vec<mir::MirTypeBridgeInitializationUnitV1>,
    strong: lir::StrongProductionSectionV2,
    ordinary: lir::CrossConeLirBridgeSectionV1,
    layout: lir::PhysicalImportsReplayedLayoutAbiSectionV1,
}

/// Read-only physical joins; Link consumption additionally checks object uses.
pub struct PhysicalImportsReplayedCrossConeLayoutClosure {
    current: ConeIdentity,
    target: lir::ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    artifacts: Vec<PhysicalImportsReplayedCrossConeLayoutSections>,
    positions: BTreeMap<ConeIdentity, usize>,
}

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub fn replay_physical_imports(
        self,
    ) -> Result<PhysicalImportsReplayedCrossConeLayoutClosure, CrossConeLayoutLirPhysicalError>
    {
        self.replay_physical(|artifact, _, _, _| {
            artifact
                .prepared
                .validate_link_materializations()
                .map_err(SharedLirPhysicalError::from)
        })
        .map(|(physical, _)| physical)
    }
}

impl PhysicalImportsReplayedCrossConeLayoutClosure {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }
    pub const fn target_selection(&self) -> lir::ValidatedLirTargetSelection {
        self.target
    }
    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }
    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &PhysicalImportsReplayedCrossConeLayoutSections> + '_ {
        self.artifacts.iter()
    }
    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&PhysicalImportsReplayedCrossConeLayoutSections> {
        self.positions
            .get(&provider)
            .map(|&index| &self.artifacts[index])
    }
}

impl PhysicalImportsReplayedCrossConeLayoutSections {
    pub fn lir_exports(&self) -> &lir::LayoutAbiExportConstituentsV1 {
        self.layout.exports()
    }
    pub fn lir_physical_imports(&self) -> &lir::CanonicalExternalShapeLinkImportsV1 {
        self.layout.physical_imports()
    }
    pub fn lir_strong_production(&self) -> &lir::StrongProductionSectionV2 {
        &self.strong
    }
}
