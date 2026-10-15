//! Saved ordinary callable bridges and their typed references.

use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    LirLayoutsValidatedCrossConeLayoutClosure, LirLayoutsValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};

mod errors;
pub use errors::{CrossConeLayoutOrdinaryLirBridgeError, SharedOrdinaryLirBridgeValidationError};

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
        for artifact in dependency_first {
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
                let ordinary = ordinary.validate(parts.identities, parts.lir_foundation)?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root())?;
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
