//! Shared MIR decoding and source agreement within each provider scope.

use std::collections::BTreeMap;

use scoop_hir::{CheckedSharedTypeFoundationV1, SharedTypeMetadataV1};
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

mod constructors;
mod equality;
mod errors;
mod objects;
mod replay;
mod validation;
use SharedMirSourceCallableValidationError as Error;
pub use constructors::{
    SharedMirConstructorComponent, SharedMirConstructorValidationError,
    validate_shared_mir_constructors,
};
pub use equality::{SharedMirEqualityValidationError, validate_shared_mir_equality};
pub use errors::{
    CrossConeLayoutMirSourceCallablesError, SharedMirSourceCallableComponent,
    SharedMirSourceCallablePartition, SharedMirSourceCallableValidationError,
};
pub use objects::{
    SharedMirObjectComponent, SharedMirObjectValidationError, validate_shared_mir_objects,
};
pub use validation::validate_shared_mir_source_callables;

struct ResolvedMirSourceSections<'input> {
    prepared: PreparedCrossConeLayoutMirSections<'input>,
    mir: CallablesResolvedCrossConeMirTypeBridgeSectionV1,
    lir: DecodedCrossConeLayoutLirCandidates,
}

/// Types, callables, object initialization, unit contracts and dispatch agree
/// with shared HIR. Initialization uses, selected uses and LIR remain unvalidated.
pub struct MirSourceCallablesValidatedCrossConeLayoutSections<'input> {
    pub(super) prepared: PreparedCrossConeLayoutMirSections<'input>,
    pub(super) mir: CallablesResolvedCrossConeMirTypeBridgeSectionV1,
    pub(super) lir: DecodedCrossConeLayoutLirCandidates,
    pub(super) units: Vec<scoop_mir::MirTypeBridgeInitializationUnitV1>,
}

pub struct MirSourceCallablesValidatedCrossConeLayoutClosure<'input> {
    pub(super) current: ConeIdentity,
    pub(super) target: ValidatedLirTargetSelection,
    pub(super) direct: Vec<ConeIdentity>,
    pub(super) dependency_first: Vec<MirSourceCallablesValidatedCrossConeLayoutSections<'input>>,
    pub(super) positions: BTreeMap<ConeIdentity, usize>,
    pub(super) dependency_positions: Vec<Vec<usize>>,
}

impl<'input> HirDeclarationsValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_mir_sources(
        self,
    ) -> Result<
        MirSourceCallablesValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutMirSourceCallablesError,
    > {
        let HirProductionValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.declarations;
        let mut resolved: Vec<ResolvedMirSourceSections<'input>> = Vec::new();
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
                Ok(ResolvedMirSourceSections { prepared, mir, lir })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutMirSourceCallablesError::new(provider, source))?;
            resolved.push(artifact);
        }
        let units = replay::validate_sources(&mut resolved, &dependency_positions)?;
        let mut complete = Vec::new();
        for (ResolvedMirSourceSections { prepared, mir, lir }, units) in
            resolved.into_iter().zip(units)
        {
            let provider = prepared.provider();
            scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root()).map_err(
                |source| CrossConeLayoutMirSourceCallablesError::new(provider, source.into()),
            )?;
            complete.push(MirSourceCallablesValidatedCrossConeLayoutSections {
                prepared,
                mir,
                lir,
                units,
            });
        }
        Ok(MirSourceCallablesValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl MirSourceCallablesValidatedCrossConeLayoutSections<'_> {
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

impl MirSourceCallablesValidatedCrossConeLayoutClosure<'_> {
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
    ) -> impl ExactSizeIterator<Item = &MirSourceCallablesValidatedCrossConeLayoutSections<'_>>
    {
        self.dependency_first.iter()
    }
    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<&MirSourceCallablesValidatedCrossConeLayoutSections<'_>> {
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
