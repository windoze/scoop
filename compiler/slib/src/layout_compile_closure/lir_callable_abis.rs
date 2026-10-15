//! Saved callable ABI records and their typed references.

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

mod errors;
pub use errors::{CrossConeLayoutLirCallableAbisError, SharedLirCallableAbiValidationError};

/// Layouts and saved callable ABI records available to import selection.
pub type LirCallableAbisValidatedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::CallablesResolvedCrossConeLayoutAbiSectionV1,
    >;
pub type LirCallableAbisValidatedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::CallablesResolvedCrossConeLayoutAbiSectionV1,
    >;

impl<'input> OrdinaryLirBridgeValidatedCrossConeLayoutClosure<'input> {
    pub fn validate_lir_callable_abis(
        self,
    ) -> Result<
        LirCallableAbisValidatedCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirCallableAbisError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirCallableAbisValidatedCrossConeLayoutSections<'input>> = Vec::new();
        for artifact in dependency_first {
            let provider = artifact.identity();
            let resolve = || -> Result<_, SharedLirCallableAbiValidationError> {
                let OrdinaryLirBridgeValidatedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let layout = layout.read_callables(
                    target.target(),
                    parts.lir_foundation,
                    parts.identities,
                )?;
                scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root())?;
                Ok(LirCallableAbisValidatedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve()
                .map_err(|source| CrossConeLayoutLirCallableAbisError::new(provider, source))?;
            complete.push(artifact);
        }
        Ok(LirCallableAbisValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirCallableAbisValidatedCrossConeLayoutSections<'_> {
    pub fn layouts(&self) -> &lir::CanonicalExactLayoutExportsV1 {
        self.layout.layouts()
    }
    pub fn callable_abis(&self) -> &lir::CanonicalExactCallableAbiExportsV1 {
        self.layout.callables()
    }
}
