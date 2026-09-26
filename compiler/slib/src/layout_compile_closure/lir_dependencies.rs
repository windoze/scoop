//! Shared materialized HIR type roots close the LIR semantic dependency graph.

use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::{
    MirDependencyGraphReplayedCrossConeLayoutClosure,
    MirDependencyGraphReplayedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::dependency_reachability::transitive_positions;

mod errors;
mod initialization;
mod replay;
pub use errors::{CrossConeLayoutLirDependenciesError, SharedLirDependencyGraphError};
pub use initialization::replay_shared_lir_initialization_dependencies;
pub use replay::replay_shared_lir_dependency_graph;

/// Complete semantic graph replay for local exports and shared type roots.
/// Physical imports, per-use source/access and final production joins remain
/// mandatory before any machine or publication capability can be constructed.
pub type LirDependencyGraphReplayedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::StrongProductionSectionV2,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;
pub type LirDependencyGraphReplayedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::StrongProductionSectionV2,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;

impl<'input> MirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub fn replay_lir_dependency_graph(
        self,
    ) -> Result<
        LirDependencyGraphReplayedCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirDependenciesError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirDependencyGraphReplayedCrossConeLayoutSections<'input>> =
            Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirDependencyGraphError> {
                let MirDependencyGraphReplayedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                replay_shared_lir_initialization_dependencies(
                    mir.exports().initialization_uses(),
                    strong.initialization_registrations(),
                )?;
                let reachable = transitive_positions(position, &dependency_positions)?;
                let layout = layout.resolve_dependencies(parts.identities)?;
                let path = WirePath::root();
                let mut dependencies = Vec::new();
                scoop_wire::allocation::try_reserve(&mut dependencies, reachable.len(), &path)?;
                dependencies.extend(
                    reachable
                        .iter()
                        .map(|&index| complete[index].layout.exports()),
                );
                replay_shared_lir_dependency_graph(
                    scoop_hir::SharedTypeMetadataV1 {
                        provider,
                        identities: parts.identities,
                        foundation: parts.hir_foundation,
                        public: parts.hir_interface,
                    },
                    &layout,
                    &dependencies,
                    strong.static_storage_registrations().registrations(),
                )?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &path)?;
                Ok(LirDependencyGraphReplayedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve().map_err(|source| CrossConeLayoutLirDependenciesError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
        }
        Ok(LirDependencyGraphReplayedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirDependencyGraphReplayedCrossConeLayoutSections<'_> {
    pub const fn lir_dependency_transport(
        &self,
    ) -> &lir::DependencyResolvedCrossConeLayoutAbiSectionV1 {
        &self.layout
    }

    pub const fn lir_exports(&self) -> &lir::LayoutAbiExportConstituentsV1 {
        self.layout.exports()
    }

    pub const fn lir_strong_production(&self) -> &lir::StrongProductionSectionV2 {
        &self.strong
    }
}
