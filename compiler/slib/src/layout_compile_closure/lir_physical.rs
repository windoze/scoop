//! Shared physical replay borrows actual dependencies inside one owned scope.

use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};
use std::{collections::BTreeMap, convert::Infallible};
use typed_arena::Arena;

use super::{
    LirDependencyGraphReplayedCrossConeLayoutClosure,
    LirDependencyGraphReplayedCrossConeLayoutSections,
    lir_constituents::LirConstituentsValidatedCrossConeLayoutSections,
};
use crate::dependency_reachability::transitive_positions;

mod driver;
mod errors;
mod objects;
mod replay;
mod support;
mod symbols;
pub use errors::{CrossConeLayoutLirPhysicalError, SharedLirPhysicalError};
pub use objects::LinkObjectsReplayedCrossConeLayoutClosure;
pub use symbols::LinkSymbolsReplayedCrossConeLayoutClosure;

pub type PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::PhysicalImportsReplayedLayoutAbiSectionV1<'checked>,
        lir::CrossConeLirBridgeSectionV1,
        lir::ReplayedStrongProductionSectionV2,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;

/// Read-only physical joins. Source/access, object-use coverage and Compile/Link
/// agreement must still succeed before complete machine consumption.
pub struct PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input> {
    current: ConeIdentity,
    target: lir::ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    artifacts: Vec<&'checked PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked>>,
    positions: BTreeMap<ConeIdentity, usize>,
}

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub fn with_replayed_physical_imports<R>(
        self,
        use_checked: impl for<'checked> FnOnce(
            PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input>,
        ) -> R,
    ) -> Result<R, CrossConeLayoutLirPhysicalError> {
        self.with_replayed_physical(
            |artifact, _, _, _| {
                artifact
                    .prepared
                    .validate_link_materializations(&artifact.strong)?;
                Ok(())
            },
            |physical, _| use_checked(physical),
        )
    }
}

impl<'checked, 'input> PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input> {
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
    ) -> impl ExactSizeIterator<
        Item = &'checked PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked>,
    > + '_ {
        self.artifacts.iter().copied()
    }
    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&'checked PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked>> {
        self.positions
            .get(&provider)
            .map(|&index| self.artifacts[index])
    }
}

impl<'checked> PhysicalImportsReplayedCrossConeLayoutSections<'_, 'checked> {
    pub fn lir_exports(&self) -> &lir::LayoutAbiExportConstituentsV1 {
        self.layout.exports()
    }
    pub fn lir_physical_imports(&self) -> &lir::CanonicalExternalShapeLinkImportsV1<'checked> {
        self.layout.physical_imports()
    }
    pub fn lir_strong_production(&self) -> &lir::ReplayedStrongProductionSectionV2 {
        &self.strong
    }
}
