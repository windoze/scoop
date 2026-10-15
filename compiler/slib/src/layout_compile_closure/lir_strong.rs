//! Read saved production inputs within the dependency closure.

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

mod errors;
pub use errors::CrossConeLayoutLirProductionDataError;

/// Saved definition and initialization data for dependency compilation.
pub type LirProductionDataCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::CompileProduction,
    >;
pub type LirProductionDataCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::CompileProduction,
    >;

impl<'input> LirExportsValidatedCrossConeLayoutClosure<'input> {
    pub fn read_lir_production_data(
        self,
    ) -> Result<
        LirProductionDataCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirProductionDataError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirProductionDataCrossConeLayoutSections<'input>> = Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, lir::LinkDataError> {
                let LirExportsValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;

                let path = WirePath::root();

                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions)
                    .map_err(|error| lir::LinkDataError(error.to_string()))?;
                let dependencies = reachable
                    .iter()
                    .map(|&index| &complete[index].strong)
                    .collect::<Vec<_>>();
                let layouts = reachable
                    .iter()
                    .map(|&index| complete[index].layout.exports().layouts())
                    .collect::<Vec<_>>();
                let strong = strong.read_compile(
                    parts.lir_foundation,
                    parts.identities,
                    &layouts,
                    &dependencies,
                )?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &path)
                    .map_err(|error| lir::LinkDataError(error.to_string()))?;
                Ok(LirProductionDataCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutLirProductionDataError { provider, source })?;
            complete.push(artifact);
        }
        Ok(LirProductionDataCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirProductionDataCrossConeLayoutSections<'_> {
    pub const fn lir_production_data(&self) -> &lir::CompileProduction {
        &self.strong
    }
}
