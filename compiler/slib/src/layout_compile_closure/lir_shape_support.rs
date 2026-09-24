//! The shared finite MIR roots close all five LIR export constituents.

use super::{
    LirDescriptorsValidatedCrossConeLayoutClosure, LirDescriptorsValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use scoop_lir as lir;
use scoop_wire::WirePath;

mod errors;
mod replay;
pub use errors::{CrossConeLayoutLirShapeSupportError, SharedLirShapeSupportValidationError};
pub use replay::replay_shared_mir_shape_support;

pub type LirExportsValidatedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
    >;
pub type LirExportsValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
    >;

impl<'input> LirDescriptorsValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_lir_shape_support(
        self,
    ) -> Result<
        LirExportsValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirShapeSupportError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete = Vec::new();
        for artifact in dependency_first {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirShapeSupportValidationError> {
                let LirDescriptorsValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let expected = replay_shared_mir_shape_support(
                    mir.shape_support(),
                    layout.layouts(),
                    layout.descriptors(),
                    parts.identities,
                    parts.lir_foundation,
                    parts.meter,
                )?;
                let layout = layout.validate_shape_support(&expected, parts.meter)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &WirePath::root())?;
                Ok(LirExportsValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve().map_err(|source| CrossConeLayoutLirShapeSupportError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
        }
        Ok(LirExportsValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl<O>
    LirConstituentsValidatedCrossConeLayoutSections<
        '_,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        O,
    >
{
    pub const fn lir_exports(&self) -> &lir::LayoutAbiExportConstituentsV1 {
        self.layout.exports()
    }
}
