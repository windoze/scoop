//! Shared HIR roots and complete MIR constituent dependency graph replay.

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
mod initialization;
mod replay;
pub use errors::{CrossConeLayoutMirDependenciesError, SharedMirDependencyGraphError};
pub use replay::replay_shared_mir_dependency_graph;

/// The recursive selected graph agrees with shared materialized type roots.
/// Per-use HIR/access and final LIR/Link joins are still
/// required; these records cannot construct an external machine arena.
pub type MirDependencyGraphReplayedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::ConeProductionSectionV2,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;
pub type MirDependencyGraphReplayedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::ConeProductionSectionV2,
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
                let reachable = transitive_positions(position, &dependency_positions)?;
                let mir = mir.resolve_dependencies(
                    mir::MirTypeBridgeLocalInputV1 {
                        provider,

                        production: parts.mir_core,
                        ordinary: parts.mir_ordinary,
                    },
                    parts.identities,
                )?;
                let path = WirePath::root();
                let mut dependencies = Vec::new();
                scoop_wire::allocation::try_reserve(&mut dependencies, reachable.len(), &path)?;
                dependencies.extend(
                    reachable
                        .iter()
                        .map(|&index| complete[index].mir.dependency_view(&complete[index].units)),
                );
                let mut source_dependencies = Vec::new();

                scoop_wire::allocation::try_reserve(
                    &mut source_dependencies,
                    reachable.len(),
                    &path,
                )?;
                source_dependencies.extend(
                    reachable
                        .iter()
                        .map(|&index| complete[index].prepared.shared_metadata()),
                );
                crate::hir_dependency_calls::validate_executable_hir_calls(
                    parts.hir_interface,
                    parts.mir_core.strong_callable_bridges(),
                    parts.mir_ordinary,
                    Some((&mir, &dependencies)),
                    parts.mir_foundation,
                    parts.identities,
                )
                .map_err(|source| SharedMirDependencyGraphError::CallSites(Box::new(source)))?;
                replay_shared_mir_dependency_graph(
                    scoop_hir::SharedTypeMetadataV1 {
                        provider,
                        identities: parts.identities,
                        foundation: parts.hir_foundation,
                        public: parts.hir_interface,
                    },
                    &source_dependencies,
                    &mir,
                    &units,
                    &dependencies,
                )?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &path)?;
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

    pub const fn lir_strong_production(&self) -> &lir::ConeProductionSectionV2 {
        &self.strong
    }
}
