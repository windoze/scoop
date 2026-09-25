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

mod errors;
mod replay;
mod support;
pub use errors::{CrossConeLayoutLirPhysicalError, SharedLirPhysicalError};

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
        let arena = Arena::new();
        let mut complete = Vec::new();
        for (position, artifact) in self.dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let replay = || -> Result<_, SharedLirPhysicalError> {
                let LirDependencyGraphReplayedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable =
                    transitive_positions(position, &self.dependency_positions, parts.meter)?;
                let mut dependencies = Vec::new();
                parts.meter.try_reserve_collection_slots(
                    &mut dependencies,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                dependencies.extend(reachable.into_iter().map(|index| complete[index]));
                let layout = replay::physical(
                    layout,
                    &strong,
                    &mir,
                    &dependencies,
                    parts.identities,
                    parts.meter,
                )?;
                strong.validate_replayed_layout_selection(&layout, parts.meter)?;
                if let Some(link) = parts.link_sections {
                    link.layout_link_closure_wire()
                        .validate_physical_imports_against(
                            layout.physical_imports(),
                            parts.meter,
                        )?;
                }
                parts.meter.charge_owned_bytes(
                    std::mem::size_of::<PhysicalImportsReplayedCrossConeLayoutSections<'_, '_>>()
                        as u64,
                    &WirePath::root(),
                )?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &WirePath::root())?;
                Ok(PhysicalImportsReplayedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = replay().map_err(|source| CrossConeLayoutLirPhysicalError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(&*arena.alloc(artifact));
        }
        Ok(use_checked(PhysicalImportsReplayedCrossConeLayoutClosure {
            current: self.current,
            target: self.target,
            direct: self.direct,
            artifacts: complete,
            positions: self.positions,
        }))
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
