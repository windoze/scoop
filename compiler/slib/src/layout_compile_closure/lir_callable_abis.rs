//! Source-derived ABI replay after shared MIR and layout validation.

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
pub use errors::{CrossConeLayoutLirCallableAbisError, SharedLirCallableAbiValidationError};
pub use replay::replay_shared_mir_callable_abis;

/// Layouts and callable ABIs agree with shared MIR. Remaining semantic,
/// selected-use, registration and object joins are not implied by this state.
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
        for (position, artifact) in dependency_first.into_iter().enumerate() {
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
                let expected = replay_shared_mir_callable_abis(
                    target.target(),
                    mir.callables(),
                    layout.layouts(),
                    &dependencies,
                    parts.lir_foundation,
                )?;
                let layout = layout.validate_callables(&expected)?;
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
