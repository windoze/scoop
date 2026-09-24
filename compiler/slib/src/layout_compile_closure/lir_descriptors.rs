//! Descriptor replay from shared types, layouts, dispatch and dependencies.

use super::{
    LirDispatchValidatedCrossConeLayoutClosure, LirDispatchValidatedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};
use crate::dependency_reachability::transitive_positions;
use scoop_identity::ExactTypeDiagnosticCatalog;
use scoop_lir as lir;
use scoop_wire::{WirePath, encoded_length};

mod errors;
mod replay;
pub use errors::{CrossConeLayoutLirDescriptorsError, SharedLirDescriptorValidationError};
pub use replay::{SharedLirDescriptorInputsV1, replay_shared_mir_descriptors};

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
        for (position, artifact) in dependency_first.into_iter().enumerate() {
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
                let length = encoded_length(prepared.coordinate())?;
                prepared
                    .semantic_parts()
                    .meter
                    .charge_owned_bytes(length, &WirePath::root())?;
                let coordinate = prepared.coordinate().clone();
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &dependency_positions, parts.meter)?;
                let mut descriptors = Vec::new();
                let mut coordinates = Vec::new();
                parts.meter.try_reserve_collection_slots(
                    &mut descriptors,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                parts.meter.try_reserve_collection_slots(
                    &mut coordinates,
                    reachable.len() + 1,
                    &WirePath::root(),
                )?;
                coordinates.push(coordinate);
                for &position in &reachable {
                    let artifact = &complete[position];
                    descriptors.push(artifact.descriptors());
                    parts.meter.charge_owned_bytes(
                        encoded_length(artifact.coordinate())?,
                        &WirePath::root(),
                    )?;
                    coordinates.push(artifact.coordinate().clone());
                }
                let diagnostics = ExactTypeDiagnosticCatalog::try_new(
                    parts.identities,
                    &coordinates,
                    parts.meter,
                )?;
                let expected = replay_shared_mir_descriptors(
                    target.target(),
                    mir.types(),
                    SharedLirDescriptorInputsV1 {
                        layouts: layout.layouts(),
                        dispatch: layout.dispatch(),
                        dependencies: &descriptors,
                    },
                    &diagnostics,
                    parts.lir_foundation,
                    parts.meter,
                )?;
                let layout = layout.validate_descriptors(&expected, parts.meter)?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &WirePath::root())?;
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
