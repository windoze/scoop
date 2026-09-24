//! Owned type replay from the same artifacts' shared HIR declarations.

use std::{collections::BTreeMap, convert::Infallible};

use scoop_hir::{CheckedSharedTypeFoundationV1, SharedTypeMetadataV1};
use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_mir::{
    CanonicalMirShapeSupportsV1, CanonicalParamFreeMirTypeExportsV1,
    TypeResolvedCrossConeMirTypeBridgeSectionV1,
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
mod helpers;
mod representation;
mod validation;
use SharedMirTypeValidationError as Error;
pub use errors::{
    CrossConeLayoutMirTypesError, SharedMirTypeComponent, SharedMirTypeValidationError,
};
pub use validation::validate_shared_mir_type_exports;

/// Type and finite-shape exports agree with shared source metadata. Callable,
/// dispatch, selected-use and LIR candidates still require their own replay.
pub struct MirTypesValidatedCrossConeLayoutSections<'input> {
    prepared: PreparedCrossConeLayoutMirSections<'input>,
    mir: TypeResolvedCrossConeMirTypeBridgeSectionV1,
    lir: DecodedCrossConeLayoutLirCandidates,
}

pub struct MirTypesValidatedCrossConeLayoutClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<MirTypesValidatedCrossConeLayoutSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

impl<'input> HirDeclarationsValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_mir_types(
        self,
    ) -> Result<MirTypesValidatedCrossConeLayoutClosure<'input>, CrossConeLayoutMirTypesError> {
        let HirProductionValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.0;
        let mut resolved: Vec<MirTypesValidatedCrossConeLayoutSections<'input>> = Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, Error> {
                let (mut prepared, mir, lir) = artifact.prepare_mir_semantics()?;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let mir = mir.resolve_types::<Infallible>(
                    provider,
                    parts.mir_foundation,
                    reachable
                        .iter()
                        .map(|position| resolved[*position].mir.types()),
                    parts.identities,
                    parts.meter,
                )?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut resolved, 1, &WirePath::root())?;
                Ok(MirTypesValidatedCrossConeLayoutSections { prepared, mir, lir })
            };
            let artifact =
                resolve().map_err(|source| CrossConeLayoutMirTypesError::new(provider, source))?;
            resolved.push(artifact);
        }
        validate_sources(&mut resolved, &dependency_positions)?;
        Ok(MirTypesValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: resolved,
            positions,
            dependency_positions,
        })
    }
}

fn validate_sources(
    artifacts: &mut [MirTypesValidatedCrossConeLayoutSections<'_>],
    dependency_positions: &[Vec<usize>],
) -> Result<(), CrossConeLayoutMirTypesError> {
    let mut checked: Vec<CheckedSharedTypeFoundationV1<'_>> = Vec::new();
    for (position, artifact) in artifacts.iter_mut().enumerate() {
        let provider = artifact.identity();
        let parts = artifact.prepared.semantic_parts();
        let mut validate = || -> Result<_, Error> {
            let reachable = transitive_positions(position, dependency_positions, parts.meter)?;
            let mut dependencies = Vec::new();
            parts.meter.try_reserve_collection_slots(
                &mut dependencies,
                reachable.len(),
                &WirePath::root(),
            )?;
            dependencies.extend(reachable.iter().map(|position| checked[*position]));
            let source = parts.hir_types.validate_shared_foundation(
                SharedTypeMetadataV1 {
                    provider,
                    identities: parts.identities,
                    foundation: parts.hir_foundation,
                    public: parts.hir_interface,
                },
                &dependencies,
                parts.meter,
            )?;
            source.with_inheritance_graph(&dependencies, parts.meter, |graph, meter| {
                validation::validate(source, graph, parts.hir_core, &artifact.mir, meter)
            })??;
            parts
                .meter
                .try_reserve_collection_slots(&mut checked, 1, &WirePath::root())?;
            Ok(source)
        };
        let source =
            validate().map_err(|source| CrossConeLayoutMirTypesError::new(provider, source))?;
        checked.push(source);
    }
    Ok(())
}

impl MirTypesValidatedCrossConeLayoutSections<'_> {
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

    pub fn lir_strong_production_wire(&self) -> &scoop_lir::DecodedStrongProductionSectionV2 {
        &self.lir.strong
    }

    pub fn lir_cross_cone_bridge_wire(&self) -> &scoop_lir::DecodedCrossConeLirBridgeSectionV1 {
        &self.lir.ordinary
    }

    pub fn lir_layout_abi_wire(&self) -> &scoop_lir::DecodedCrossConeLayoutAbiSectionV1 {
        &self.lir.layout
    }
}

impl MirTypesValidatedCrossConeLayoutClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &MirTypesValidatedCrossConeLayoutSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&MirTypesValidatedCrossConeLayoutSections<'_>> {
        self.positions
            .get(&provider)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn dependency_count(&self, provider: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&provider)
            .map(|position| self.dependency_positions[*position].len())
    }
}
