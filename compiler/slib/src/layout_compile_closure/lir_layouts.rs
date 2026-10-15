//! Saved layout data in the owned Compile closure.

use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    MirRecordsCrossConeLayoutClosure, MirRecordsCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::layout_compile_decode::DecodedCrossConeLayoutLirCandidates;

mod errors;
pub use errors::{CrossConeLayoutLirLayoutsError, SharedLirLayoutValidationError};

/// Loaded layout records retained with their owning artifact.
pub type LirLayoutsValidatedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::LayoutsResolvedCrossConeLayoutAbiSectionV1,
        lir::DecodedCrossConeLirBridgeSectionV1,
    >;
pub type LirLayoutsValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::LayoutsResolvedCrossConeLayoutAbiSectionV1,
        lir::DecodedCrossConeLirBridgeSectionV1,
    >;

impl<'input> MirRecordsCrossConeLayoutClosure<'input> {
    pub fn validate_lir_layouts(
        self,
    ) -> Result<LirLayoutsValidatedCrossConeLayoutClosure<'input>, CrossConeLayoutLirLayoutsError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirLayoutsValidatedCrossConeLayoutSections<'input>> = Vec::new();
        for artifact in dependency_first {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirLayoutValidationError> {
                let MirRecordsCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    lir,
                    units,
                } = artifact;
                let parts = prepared.semantic_parts();
                let DecodedCrossConeLayoutLirCandidates {
                    strong,
                    ordinary,
                    layout,
                } = lir;
                let layout =
                    layout.read_layouts(target.target(), parts.lir_foundation, parts.identities)?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root())?;
                Ok(LirLayoutsValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutLirLayoutsError::new(provider, source))?;
            complete.push(artifact);
        }
        Ok(LirLayoutsValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirLayoutsValidatedCrossConeLayoutSections<'_> {
    pub fn layouts(&self) -> &lir::CanonicalExactLayoutExportsV1 {
        self.layout.layouts()
    }
}
