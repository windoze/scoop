//! Layout replay in the same owned Compile closure as shared HIR and MIR.

use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    MirSourceCallablesValidatedCrossConeLayoutClosure,
    MirSourceCallablesValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::{
    dependency_reachability::transitive_positions,
    layout_compile_decode::DecodedCrossConeLayoutLirCandidates,
};

mod errors;
mod replay;
pub use errors::{CrossConeLayoutLirLayoutsError, SharedLirLayoutValidationError};
pub use replay::replay_shared_mir_layouts;

/// HIR, MIR source constituents and LIR layouts agree. Selected-use, remaining
/// LIR exports, Strong production and final object joins remain mandatory.
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

impl<'input> MirSourceCallablesValidatedCrossConeLayoutClosure<'input> {
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
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirLayoutValidationError> {
                let MirSourceCallablesValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    lir,
                    units,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions)?;
                let mut dependencies = Vec::new();
                scoop_wire::allocation::try_reserve(
                    &mut dependencies,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                dependencies.extend(
                    reachable
                        .iter()
                        .map(|&position| complete[position].layouts()),
                );
                let expected = replay_shared_mir_layouts(
                    target.target(),
                    mir.types(),
                    parts.lir_foundation,
                    parts.identities,
                    &dependencies,
                )?;
                let DecodedCrossConeLayoutLirCandidates {
                    strong,
                    ordinary,
                    layout,
                } = lir;
                let layout = layout.validate_layouts(&expected)?;
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
