//! Dispatch replay from the shared MIR schemas and checked LIR ABIs.

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
mod replay;
pub use errors::{CrossConeLayoutLirDispatchError, SharedLirDispatchValidationError};
pub use replay::{SharedLirDispatchAbiInputsV1, replay_shared_mir_dispatch};

/// Layouts, callable ABIs and dispatch agree with shared MIR. Descriptor,
/// selected-use, registration and object joins remain separate validations.
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
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let mut layouts = Vec::new();
                let mut callables = Vec::new();
                parts.meter.try_reserve_collection_slots(
                    &mut layouts,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                parts.meter.try_reserve_collection_slots(
                    &mut callables,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                for &position in &reachable {
                    layouts.push(complete[position].layouts());
                    callables.push(complete[position].callable_abis());
                }
                let expected = replay_shared_mir_dispatch(
                    target.target(),
                    mir.types(),
                    mir.dispatch(),
                    SharedLirDispatchAbiInputsV1 {
                        local_layouts: layout.layouts(),
                        local_callables: layout.callables(),
                        dependency_layouts: &layouts,
                        dependency_callables: &callables,
                    },
                    parts.lir_foundation,
                    parts.meter,
                )?;
                let layout = layout.validate_dispatch(&expected, parts.meter)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &WirePath::root())?;
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
