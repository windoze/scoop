//! Saved dispatch records and their referenced receiver layouts.

use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    LirCallableAbisValidatedCrossConeLayoutClosure,
    LirCallableAbisValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::dependency_reachability::transitive_positions;

mod errors;
pub use errors::{CrossConeLayoutLirDispatchError, SharedLirDispatchValidationError};

/// Parsed layouts, callable ABIs and dispatch records.
pub type LirDispatchValidatedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::DispatchResolvedCrossConeLayoutAbiSectionV1,
    >;
pub type LirDispatchValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::DispatchResolvedCrossConeLayoutAbiSectionV1,
    >;

impl<'input> LirCallableAbisValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_lir_dispatch(
        self,
    ) -> Result<LirDispatchValidatedCrossConeLayoutClosure<'input>, CrossConeLayoutLirDispatchError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirDispatchValidatedCrossConeLayoutSections<'input>> = Vec::new();
        for (position, artifact) in dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirDispatchValidationError> {
                let LirCallableAbisValidatedCrossConeLayoutSections {
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
                layouts.extend(
                    reachable
                        .iter()
                        .map(|&position| complete[position].layouts()),
                );
                let layout = layout.read_dispatch(
                    target.target(),
                    parts.lir_foundation,
                    &layouts,
                    parts.identities,
                )?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root())?;
                Ok(LirDispatchValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutLirDispatchError::new(provider, source))?;
            complete.push(artifact);
        }
        Ok(LirDispatchValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirDispatchValidatedCrossConeLayoutSections<'_> {
    pub fn layouts(&self) -> &lir::CanonicalExactLayoutExportsV1 {
        self.layout.layouts()
    }
    pub fn callable_abis(&self) -> &lir::CanonicalExactCallableAbiExportsV1 {
        self.layout.callables()
    }
    pub fn lir_dispatch(&self) -> &lir::CanonicalExactDispatchExportsV1 {
        self.layout.dispatch()
    }
}
