//! Saved MIR records and typed references within each provider scope.

use std::collections::BTreeMap;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_mir::{
    CallablesResolvedCrossConeMirTypeBridgeSectionV1, CanonicalMirCallableBindingsV1,
    CanonicalMirDispatchSchemasV1, CanonicalMirObjectValuesV1, CanonicalMirShapeSupportsV1,
    CanonicalParamFreeMirTypeExportsV1,
};
use scoop_wire::WirePath;

use super::{
    HirDeclarationsValidatedCrossConeLayoutClosure, HirProductionValidatedCrossConeLayoutClosure,
};
use crate::{
    dependency_reachability::transitive_positions,
    layout_compile_decode::{
        DecodedCrossConeLayoutLirCandidates, PreparedCrossConeLayoutMirSections,
    },
};

mod errors;
use SharedMirReadError as Error;
pub use errors::{CrossConeLayoutMirReadError, SharedMirReadError};

/// Parsed MIR records and initialization pairs available to later consumers.
pub struct MirRecordsCrossConeLayoutSections<'input> {
    pub(super) prepared: PreparedCrossConeLayoutMirSections<'input>,
    pub(super) mir: CallablesResolvedCrossConeMirTypeBridgeSectionV1,
    pub(super) lir: DecodedCrossConeLayoutLirCandidates,
    pub(super) units: Vec<scoop_mir::MirTypeBridgeInitializationUnitV1>,
}

pub struct MirRecordsCrossConeLayoutClosure<'input> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) dependency_first: Vec<MirRecordsCrossConeLayoutSections<'input>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
    pub(super) dependency_positions: Vec<Vec<usize>>,
}

impl<'input> HirDeclarationsValidatedCrossConeLayoutClosure<'input> {
    pub fn read_mir_records(
        self,
    ) -> Result<MirRecordsCrossConeLayoutClosure<'input>, CrossConeLayoutMirReadError> {
        let HirProductionValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.declarations;
        let mut resolved: Vec<MirRecordsCrossConeLayoutSections<'input>> = Vec::new();
        for (position, (artifact, aliases)) in
            dependency_first.into_iter().zip(self.aliases).enumerate()
        {
            let provider = artifact.identity();
            let resolve = || -> Result<_, Error> {
                let (mut prepared, mir, lir) = artifact.prepare_mir_semantics(aliases)?;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions)?;
                let mir = mir.resolve_types(
                    provider,
                    parts.mir_foundation,
                    reachable
                        .iter()
                        .map(|position| resolved[*position].mir.types()),
                    parts.identities,
                )?;
                let direct_callables = std::iter::once(parts.mir_ordinary)
                    .chain(
                        reachable
                            .iter()
                            .map(|position| resolved[*position].prepared.direct_callables()),
                    )
                    .collect::<Vec<_>>();
                let mir = mir.resolve_callables(
                    parts.mir_foundation,
                    &direct_callables,
                    reachable.iter().map(|position| {
                        let mir = &resolved[*position].mir;
                        (mir.types(), mir.callables(), mir.dispatch())
                    }),
                    parts.identities,
                )?;
                scoop_wire::allocation::try_reserve(&mut resolved, 1, &WirePath::root())?;
                let units = scoop_mir::read_initialization_units(
                    parts.mir_core.strong_callable_bridges(),
                    parts.identities,
                )?;
                Ok(MirRecordsCrossConeLayoutSections {
                    prepared,
                    mir,
                    lir,
                    units,
                })
            };
            let artifact =
                resolve().map_err(|source| CrossConeLayoutMirReadError::new(provider, source))?;
            resolved.push(artifact);
        }
        Ok(MirRecordsCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: resolved,
            positions,
            dependency_positions,
        })
    }
}

impl MirRecordsCrossConeLayoutSections<'_> {
    pub fn identity(&self) -> ConeIdentity {
        self.prepared.provider()
    }
    pub fn coordinate(&self) -> &ConeCoordinate {
        self.prepared.coordinate()
    }
    pub fn types(&self) -> &CanonicalParamFreeMirTypeExportsV1 {
        self.mir.types()
    }
    pub fn shape_support(&self) -> &CanonicalMirShapeSupportsV1 {
        self.mir.shape_support()
    }
    pub fn callables(&self) -> &CanonicalMirCallableBindingsV1 {
        self.mir.callables()
    }
    pub fn object_values(&self) -> &CanonicalMirObjectValuesV1 {
        self.mir.object_values()
    }
    pub fn dispatch(&self) -> &CanonicalMirDispatchSchemasV1 {
        self.mir.dispatch()
    }
    pub fn initialization_units(&self) -> &[scoop_mir::MirTypeBridgeInitializationUnitV1] {
        &self.units
    }
    pub fn lir_strong_production_wire(&self) -> &scoop_lir::DecodedConeProductionSectionV2 {
        &self.lir.strong
    }
    pub fn lir_cross_cone_bridge_wire(&self) -> &scoop_lir::DecodedCrossConeLirBridgeSectionV1 {
        &self.lir.ordinary
    }
    pub fn lir_layout_abi_wire(&self) -> &scoop_lir::DecodedCrossConeLayoutAbiSectionV1 {
        &self.lir.layout
    }
}

impl MirRecordsCrossConeLayoutClosure<'_> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }
    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }
    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }
    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &MirRecordsCrossConeLayoutSections<'_>> {
        self.dependency_first.iter()
    }
    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&MirRecordsCrossConeLayoutSections<'_>> {
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
