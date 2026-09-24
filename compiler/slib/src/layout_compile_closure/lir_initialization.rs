//! Initialization-role ABI replay after the shared MIR and LIR constituent joins.

use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    OrdinaryLirBridgeValidatedCrossConeLayoutClosure,
    OrdinaryLirBridgeValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::dependency_reachability::transitive_positions;

mod errors;
mod replay;
pub use errors::{
    CrossConeLayoutLirInitializationAbiError, SharedLirInitializationAbiValidationError,
};
pub use replay::replay_shared_initialization_abi;

pub type LirInitializationAbiValidatedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::InitializationAbiResolvedStrongProductionSectionV2,
    >;
pub type LirInitializationAbiValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::InitializationAbiResolvedStrongProductionSectionV2,
    >;

impl<'input> OrdinaryLirBridgeValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_lir_initialization_abi(
        self,
    ) -> Result<
        LirInitializationAbiValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirInitializationAbiError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirInitializationAbiValidatedCrossConeLayoutSections<'input>> =
            Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirInitializationAbiValidationError> {
                let OrdinaryLirBridgeValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let path = WirePath::root();
                let mut layouts = Vec::new();
                parts
                    .meter
                    .try_reserve_collection_slots(&mut layouts, reachable.len(), &path)?;
                for position in reachable {
                    layouts.push(complete[position].lir_exports().layouts());
                }
                let expected = replay_shared_initialization_abi(
                    target.target(),
                    parts.mir_core.strong_callable_bridges(),
                    layout.exports().layouts(),
                    &layouts,
                    parts.lir_foundation,
                    parts.meter,
                )?;
                let strong = strong.validate_initialization_abi(expected, parts.meter)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &path)?;
                Ok(LirInitializationAbiValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact =
                resolve().map_err(|source| CrossConeLayoutLirInitializationAbiError {
                    provider,
                    source: Box::new(source),
                })?;
            complete.push(artifact);
        }
        Ok(LirInitializationAbiValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirInitializationAbiValidatedCrossConeLayoutSections<'_> {
    pub fn initialization_cycle_abi(&self) -> Option<&lir::CallableAbiRecordV1> {
        self.strong.initialization_cycle_abi()
    }
}
