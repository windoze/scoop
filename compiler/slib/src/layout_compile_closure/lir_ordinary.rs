//! Ordinary callable replay from the shared HIR/MIR and layout constituents.

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
mod replay;
pub use errors::{CrossConeLayoutOrdinaryLirBridgeError, SharedOrdinaryLirBridgeValidationError};
pub use replay::{SharedOrdinaryLirBridgeDependenciesV1, replay_shared_ordinary_lir_bridge};

pub type OrdinaryLirBridgeValidatedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
    >;
pub type OrdinaryLirBridgeValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
    >;

impl<'input> LirExportsValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_ordinary_lir_bridges(
        self,
    ) -> Result<
        OrdinaryLirBridgeValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutOrdinaryLirBridgeError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<OrdinaryLirBridgeValidatedCrossConeLayoutSections<'input>> =
            Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedOrdinaryLirBridgeValidationError> {
                let LirExportsValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let mut layouts = Vec::new();
                let mut callables = Vec::new();
                let mut metadata = Vec::new();
                let path = WirePath::root();
                parts
                    .meter
                    .try_reserve_collection_slots(&mut layouts, reachable.len(), &path)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut callables, reachable.len(), &path)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut metadata, reachable.len(), &path)?;
                for position in reachable {
                    let dependency = &complete[position];
                    layouts.push(dependency.lir_exports().layouts());
                    callables.push(dependency.lir_cross_cone_bridge());
                    metadata.push(dependency.prepared.shared_metadata());
                }
                let expected = replay_shared_ordinary_lir_bridge(
                    target.target(),
                    scoop_hir::SharedTypeMetadataV1 {
                        provider,
                        identities: parts.identities,
                        foundation: parts.hir_foundation,
                        public: parts.hir_interface,
                    },
                    parts.mir_ordinary,
                    layout.exports().layouts(),
                    SharedOrdinaryLirBridgeDependenciesV1 {
                        metadata: &metadata,
                        layouts: &layouts,
                        callables: &callables,
                    },
                    parts.lir_foundation,
                    parts.meter,
                )?;
                let ordinary = ordinary.validate_against(expected, parts.meter)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &path)?;
                Ok(OrdinaryLirBridgeValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve().map_err(|source| CrossConeLayoutOrdinaryLirBridgeError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
        }
        Ok(OrdinaryLirBridgeValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl OrdinaryLirBridgeValidatedCrossConeLayoutSections<'_> {
    pub const fn lir_cross_cone_bridge(&self) -> &lir::CrossConeLirBridgeSectionV1 {
        &self.ordinary
    }
}
