//! Complete Strong V2 physical replay within the owned dependency closure.

use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    LirExportsValidatedCrossConeLayoutClosure, LirExportsValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::dependency_reachability::transitive_positions;

mod dependencies;
mod errors;
mod functions;
mod initialization;
mod replay;
pub use errors::{CrossConeLayoutLirStrongProductionError, SharedLirStrongProductionError};

/// Physical replay retains the unresolved selected/layout transport. It is
/// not a publishable Compile capability or a committed production section.
pub type LirStrongProductionReplayedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::ConeProductionSectionV2,
    >;
pub type LirStrongProductionReplayedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::ConeProductionSectionV2,
    >;

impl<'input> LirExportsValidatedCrossConeLayoutClosure<'input> {
    pub fn replay_lir_strong_production(
        self,
    ) -> Result<
        LirStrongProductionReplayedCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirStrongProductionError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirStrongProductionReplayedCrossConeLayoutSections<'input>> =
            Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirStrongProductionError> {
                let LirExportsValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;

                let path = WirePath::root();

                let coordinate = prepared.coordinate();
                let coordinate = coordinate.clone();
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions)?;
                let mut dependencies = Vec::new();
                scoop_wire::allocation::try_reserve(&mut dependencies, reachable.len(), &path)?;
                dependencies.extend(reachable.iter().map(|&index| &complete[index]));
                let mut direct_providers = Vec::new();
                scoop_wire::allocation::try_reserve(
                    &mut direct_providers,
                    dependency_positions[position].len(),
                    &path,
                )?;
                direct_providers.extend(
                    dependency_positions[position]
                        .iter()
                        .map(|&index| complete[index].identity()),
                );
                let strong = replay::replay(
                    coordinate,
                    &direct_providers,
                    target.target(),
                    strong,
                    parts,
                    &dependencies,
                )?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &path)?;
                Ok(LirStrongProductionReplayedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve().map_err(|source| CrossConeLayoutLirStrongProductionError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
        }
        Ok(LirStrongProductionReplayedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirStrongProductionReplayedCrossConeLayoutSections<'_> {
    pub const fn lir_strong_production(&self) -> &lir::ConeProductionSectionV2 {
        &self.strong
    }
}
