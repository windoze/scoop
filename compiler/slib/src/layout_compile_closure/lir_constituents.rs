//! Ownership shared by the successive LIR constituent validation states.

use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir as lir;
use scoop_mir as mir;

use crate::layout_compile_decode::PreparedCrossConeLayoutMirSections;

pub struct LirConstituentsValidatedCrossConeLayoutSections<
    'input,
    L,
    O = lir::DecodedCrossConeLirBridgeSectionV1,
    S = lir::DecodedStrongProductionSectionV2,
    M = mir::CallablesResolvedCrossConeMirTypeBridgeSectionV1,
> {
    pub(super) prepared: PreparedCrossConeLayoutMirSections<'input>,
    pub(super) mir: M,
    pub(super) units: Vec<mir::MirTypeBridgeInitializationUnitV1>,
    pub(super) strong: S,
    pub(super) ordinary: O,
    pub(super) layout: L,
}

pub struct LirConstituentsValidatedCrossConeLayoutClosure<
    'input,
    L,
    O = lir::DecodedCrossConeLirBridgeSectionV1,
    S = lir::DecodedStrongProductionSectionV2,
    M = mir::CallablesResolvedCrossConeMirTypeBridgeSectionV1,
> {
    pub(super) current: ConeIdentity,
    pub(super) target: lir::ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) dependency_first:
        Vec<LirConstituentsValidatedCrossConeLayoutSections<'input, L, O, S, M>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
    pub(super) dependency_positions: Vec<Vec<usize>>,
}

impl<L, O, S, M> LirConstituentsValidatedCrossConeLayoutSections<'_, L, O, S, M> {
    pub fn link_sections(&self) -> Option<&crate::DecodedCrossConeLayoutLinkOnlySections> {
        self.prepared.link_sections()
    }
    pub fn decode_usage(&self) -> scoop_wire::DecodeUsage {
        self.prepared.decode_usage()
    }
    pub fn identity(&self) -> ConeIdentity {
        self.prepared.provider()
    }
    pub fn coordinate(&self) -> &ConeCoordinate {
        self.prepared.coordinate()
    }
    pub fn initialization_units(&self) -> &[mir::MirTypeBridgeInitializationUnitV1] {
        &self.units
    }
    pub fn lir_strong_production_wire(&self) -> &S {
        &self.strong
    }
    pub fn lir_cross_cone_bridge_wire(&self) -> &O {
        &self.ordinary
    }
    pub const fn lir_layout_abi_wire(&self) -> &L {
        &self.layout
    }
}

impl<L, O, S> LirConstituentsValidatedCrossConeLayoutSections<'_, L, O, S> {
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
}

impl<'input, L, O, S, M> LirConstituentsValidatedCrossConeLayoutClosure<'input, L, O, S, M> {
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
        Item = &LirConstituentsValidatedCrossConeLayoutSections<'input, L, O, S, M>,
    > {
        self.dependency_first.iter()
    }
    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&LirConstituentsValidatedCrossConeLayoutSections<'input, L, O, S, M>> {
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
