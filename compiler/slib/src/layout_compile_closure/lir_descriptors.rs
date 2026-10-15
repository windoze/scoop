//! Saved descriptor references and instance data.

use super::{
    LirDispatchValidatedCrossConeLayoutClosure, LirDispatchValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use scoop_lir as lir;
use scoop_wire::WirePath;

mod errors;
pub use errors::{CrossConeLayoutLirDescriptorsError, SharedLirDescriptorValidationError};

pub type LirDescriptorsValidatedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::DescriptorsResolvedCrossConeLayoutAbiSectionV1,
    >;
pub type LirDescriptorsValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::DescriptorsResolvedCrossConeLayoutAbiSectionV1,
    >;

impl<'input> LirDispatchValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_lir_descriptors(
        self,
    ) -> Result<
        LirDescriptorsValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirDescriptorsError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirDescriptorsValidatedCrossConeLayoutSections<'input>> = Vec::new();
        for artifact in dependency_first {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirDescriptorValidationError> {
                let LirDispatchValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;

                let parts = prepared.semantic_parts();
                let layout = layout.read_descriptors(
                    target.target(),
                    parts.lir_foundation,
                    parts.identities,
                )?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root())?;
                Ok(LirDescriptorsValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutLirDescriptorsError::new(provider, source))?;
            complete.push(artifact);
        }
        Ok(LirDescriptorsValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirDescriptorsValidatedCrossConeLayoutSections<'_> {
    pub fn layouts(&self) -> &lir::CanonicalExactLayoutExportsV1 {
        self.layout.layouts()
    }
    pub fn callable_abis(&self) -> &lir::CanonicalExactCallableAbiExportsV1 {
        self.layout.callables()
    }
    pub fn lir_dispatch(&self) -> &lir::CanonicalExactDispatchExportsV1 {
        self.layout.dispatch()
    }
    pub fn descriptors(&self) -> &lir::CanonicalExactDescriptorExportsV1 {
        self.layout.descriptors()
    }
}
