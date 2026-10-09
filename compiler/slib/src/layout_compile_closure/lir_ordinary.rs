//! Ordinary callable replay from the shared HIR/MIR and layout constituents.

use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    LirLayoutsValidatedCrossConeLayoutClosure, LirLayoutsValidatedCrossConeLayoutSections,
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
        lir::LayoutsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
    >;
pub type OrdinaryLirBridgeValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::LayoutsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
    >;

impl<'input> LirLayoutsValidatedCrossConeLayoutClosure<'input> {
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
                let LirLayoutsValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions)?;
                let mut layouts = Vec::new();
                let mut callables = Vec::new();
                let mut metadata = Vec::new();
                let path = WirePath::root();
                scoop_wire::allocation::try_reserve(&mut layouts, reachable.len(), &path)?;
                scoop_wire::allocation::try_reserve(&mut callables, reachable.len(), &path)?;
                scoop_wire::allocation::try_reserve(&mut metadata, reachable.len(), &path)?;
                for position in reachable {
                    let dependency = &complete[position];
                    layouts.push(dependency.layout.layouts());
                    callables.push(dependency.lir_cross_cone_bridge());
                    metadata.push(dependency.prepared.shared_metadata());
                }
                let ordinary = ordinary.validate(parts.identities, parts.lir_foundation)?;
                let expected = replay_shared_ordinary_lir_bridge(
                    target.target(),
                    scoop_hir::SharedTypeMetadataV1 {
                        provider,
                        identities: parts.identities,
                        foundation: parts.hir_foundation,
                        public: parts.hir_interface,
                    },
                    parts.mir_ordinary,
                    layout.layouts(),
                    SharedOrdinaryLirBridgeDependenciesV1 {
                        metadata: &metadata,
                        layouts: &layouts,
                        callables: &callables,
                    },
                    parts.lir_foundation,
                    &ordinary,
                )?;
                if ordinary != expected {
                    return Err(lir::CrossConeLirBridgeValidationError::SectionMismatch.into());
                }
                scoop_wire::allocation::try_reserve(&mut complete, 1, &path)?;
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

impl<L, S>
    LirConstituentsValidatedCrossConeLayoutSections<'_, L, lir::CrossConeLirBridgeSectionV1, S>
{
    pub const fn lir_cross_cone_bridge(&self) -> &lir::CrossConeLirBridgeSectionV1 {
        &self.ordinary
    }
}
