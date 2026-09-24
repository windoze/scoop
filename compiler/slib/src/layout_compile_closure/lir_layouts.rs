//! Layout replay in the same owned Compile closure as shared HIR and MIR.

use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::{
    MirSourceCallablesValidatedCrossConeLayoutClosure,
    MirSourceCallablesValidatedCrossConeLayoutSections,
};
use crate::{
    dependency_reachability::transitive_positions,
    layout_compile_decode::{
        DecodedCrossConeLayoutLirCandidates, PreparedCrossConeLayoutMirSections,
    },
};

mod errors;
mod replay;
pub use errors::{CrossConeLayoutLirLayoutsError, SharedLirLayoutValidationError};
pub use replay::replay_shared_mir_layouts;

/// HIR, MIR source constituents and LIR layouts agree. Selected-use, remaining
/// LIR exports, Strong production and final object joins remain mandatory.
pub struct LirLayoutsValidatedCrossConeLayoutSections<'input> {
    prepared: PreparedCrossConeLayoutMirSections<'input>,
    mir: mir::CallablesResolvedCrossConeMirTypeBridgeSectionV1,
    units: Vec<mir::MirTypeBridgeInitializationUnitV1>,
    strong: lir::DecodedStrongProductionSectionV2,
    ordinary: lir::DecodedCrossConeLirBridgeSectionV1,
    layout: lir::LayoutsResolvedCrossConeLayoutAbiSectionV1,
}

pub struct LirLayoutsValidatedCrossConeLayoutClosure<'input> {
    current: ConeIdentity,
    target: lir::ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<LirLayoutsValidatedCrossConeLayoutSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl<'input> MirSourceCallablesValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_lir_layouts(
        self,
    ) -> Result<LirLayoutsValidatedCrossConeLayoutClosure<'input>, CrossConeLayoutLirLayoutsError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirLayoutsValidatedCrossConeLayoutSections<'input>> = Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirLayoutValidationError> {
                let MirSourceCallablesValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    lir,
                    units,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let mut dependencies = Vec::new();
                parts.meter.try_reserve_collection_slots(
                    &mut dependencies,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                dependencies.extend(
                    reachable
                        .iter()
                        .map(|&position| complete[position].layouts()),
                );
                let expected = replay_shared_mir_layouts(
                    target.target(),
                    mir.types(),
                    parts.lir_foundation,
                    parts.identities,
                    &dependencies,
                    parts.meter,
                )?;
                let DecodedCrossConeLayoutLirCandidates {
                    strong,
                    ordinary,
                    layout,
                } = lir;
                let layout = layout.validate_layouts(&expected, parts.meter)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &WirePath::root())?;
                Ok(LirLayoutsValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutLirLayoutsError::new(provider, source))?;
            complete.push(artifact);
        }
        Ok(LirLayoutsValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirLayoutsValidatedCrossConeLayoutSections<'_> {
    pub fn identity(&self) -> ConeIdentity {
        self.prepared.provider()
    }
    pub fn coordinate(&self) -> &ConeCoordinate {
        self.prepared.coordinate()
    }
    pub fn types(&self) -> &mir::CanonicalParamFreeMirTypeExportsV1 {
        self.mir.types()
    }
    pub fn shape_support(&self) -> &mir::CanonicalMirShapeSupportsV1 {
        self.mir.shape_support()
    }
    pub fn callables(&self) -> &mir::CanonicalMirCallableBindingsV1 {
        self.mir.callables()
    }
    pub fn object_values(&self) -> &mir::CanonicalMirObjectValuesV1 {
        self.mir.object_values()
    }
    pub fn dispatch(&self) -> &mir::CanonicalMirDispatchSchemasV1 {
        self.mir.dispatch()
    }
    pub fn initialization_units(&self) -> &[mir::MirTypeBridgeInitializationUnitV1] {
        &self.units
    }
    pub fn layouts(&self) -> &lir::CanonicalExactLayoutExportsV1 {
        self.layout.layouts()
    }
    pub fn lir_strong_production_wire(&self) -> &lir::DecodedStrongProductionSectionV2 {
        &self.strong
    }
    pub fn lir_cross_cone_bridge_wire(&self) -> &lir::DecodedCrossConeLirBridgeSectionV1 {
        &self.ordinary
    }
    pub fn lir_layout_abi_wire(&self) -> &lir::LayoutsResolvedCrossConeLayoutAbiSectionV1 {
        &self.layout
    }
}

impl LirLayoutsValidatedCrossConeLayoutClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &LirLayoutsValidatedCrossConeLayoutSections<'_>> {
        self.dependency_first.iter()
    }
    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&LirLayoutsValidatedCrossConeLayoutSections<'_>> {
        self.positions
            .get(&provider)
            .map(|&position| &self.dependency_first[position])
    }
    pub fn dependency_count(&self, provider: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&provider)
            .map(|&position| self.dependency_positions[position].len())
    }
}
