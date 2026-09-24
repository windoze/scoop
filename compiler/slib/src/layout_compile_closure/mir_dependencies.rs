//! Shared HIR roots and complete MIR constituent dependency graph replay.

use std::convert::Infallible;

use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::{
    LirStrongProductionReplayedCrossConeLayoutClosure,
    LirStrongProductionReplayedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::dependency_reachability::transitive_positions;

mod errors;
pub use errors::{CrossConeLayoutMirDependenciesError, SharedMirDependencyGraphError};

/// The recursive selected graph agrees with shared materialized type roots.
/// Per-use HIR/access, initialization-use and final LIR/Link joins are still
/// required; these records cannot construct an external machine arena.
pub type MirDependencyGraphReplayedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::ReplayedStrongProductionSectionV2,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;
pub type MirDependencyGraphReplayedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::ReplayedStrongProductionSectionV2,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;

impl<'input> LirStrongProductionReplayedCrossConeLayoutClosure<'input> {
    pub fn replay_mir_dependency_graph(
        self,
    ) -> Result<
        MirDependencyGraphReplayedCrossConeLayoutClosure<'input>,
        CrossConeLayoutMirDependenciesError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<MirDependencyGraphReplayedCrossConeLayoutSections<'input>> =
            Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedMirDependencyGraphError> {
                let LirStrongProductionReplayedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let mir = mir.resolve_dependencies::<Infallible>(
                    mir::MirTypeBridgeLocalAuthorityV1::Reader {
                        provider,
                        foundation: parts.mir_foundation,
                        production: parts.mir_core,
                        ordinary: parts.mir_ordinary,
                    },
                    parts.identities,
                    parts.meter,
                )?;
                let roots = parts
                    .hir_interface
                    .external_references()
                    .materialized_type_dependencies(provider, parts.identities, parts.meter)
                    .map_err(|source| {
                        SharedMirDependencyGraphError::TypeOccurrences(Box::new(source))
                    })?;
                let path = WirePath::root();
                let mut committed = Vec::new();
                parts
                    .meter
                    .try_reserve_collection_slots(&mut committed, roots.len(), &path)?;
                committed.extend(roots.into_iter().map(|(provider, exact)| {
                    mir::MirTypeBridgeDependencyV1::new(
                        provider,
                        mir::MirTypeBridgeTargetV1::Type(exact),
                    )
                }));
                let mut dependencies = Vec::new();
                parts.meter.try_reserve_collection_slots(
                    &mut dependencies,
                    reachable.len(),
                    &path,
                )?;
                dependencies.extend(
                    reachable
                        .iter()
                        .map(|&index| complete[index].mir.dependency_view(&complete[index].units)),
                );
                mir.replay_dependency_closure::<Infallible>(
                    &units,
                    &dependencies,
                    &committed,
                    parts.identities,
                    parts.meter,
                )?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &path)?;
                Ok(MirDependencyGraphReplayedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve().map_err(|source| CrossConeLayoutMirDependenciesError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
        }
        Ok(MirDependencyGraphReplayedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl MirDependencyGraphReplayedCrossConeLayoutSections<'_> {
    pub const fn mir_dependency_transport(
        &self,
    ) -> &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
        &self.mir
    }

    pub const fn lir_strong_production(&self) -> &lir::ReplayedStrongProductionSectionV2 {
        &self.strong
    }
}
