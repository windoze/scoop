//! Saved MIR dependency records for import selection.

use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::{
    LirProductionDataCrossConeLayoutClosure, LirProductionDataCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};

mod errors;
pub use errors::{CrossConeLayoutMirDependenciesError, MirDependencyReadError};

/// Parsed MIR dependency records used by later import selection.
pub type MirDependenciesResolvedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::CompileProduction,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;
pub type MirDependenciesResolvedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::ExportsResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::CompileProduction,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;

impl<'input> LirProductionDataCrossConeLayoutClosure<'input> {
    pub fn resolve_mir_dependencies(
        self,
    ) -> Result<
        MirDependenciesResolvedCrossConeLayoutClosure<'input>,
        CrossConeLayoutMirDependenciesError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<MirDependenciesResolvedCrossConeLayoutSections<'input>> = Vec::new();
        for artifact in dependency_first {
            let provider = artifact.identity();
            let resolve = || -> Result<_, MirDependencyReadError> {
                let LirProductionDataCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let mir = mir.resolve_dependencies(
                    mir::MirTypeBridgeLocalInputV1 {
                        provider,

                        production: parts.mir_core,
                        ordinary: parts.mir_ordinary,
                    },
                    parts.identities,
                )?;
                let path = WirePath::root();
                scoop_wire::allocation::try_reserve(&mut complete, 1, &path)?;
                Ok(MirDependenciesResolvedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve().map_err(|source| CrossConeLayoutMirDependenciesError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
        }
        Ok(MirDependenciesResolvedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl MirDependenciesResolvedCrossConeLayoutSections<'_> {
    pub const fn mir_dependency_transport(
        &self,
    ) -> &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
        &self.mir
    }

    pub const fn lir_production_data(&self) -> &lir::CompileProduction {
        &self.strong
    }
}
