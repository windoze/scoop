//! Read saved LIR dependency references for subsequent import selection.

use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::{
    MirDependenciesResolvedCrossConeLayoutClosure, MirDependenciesResolvedCrossConeLayoutSections,
    lir_constituents::{
        LirConstituentsValidatedCrossConeLayoutClosure,
        LirConstituentsValidatedCrossConeLayoutSections,
    },
};

mod errors;
pub use errors::{CrossConeLayoutLirDependenciesError, LirDependencyReadError};

pub type LirDependenciesResolvedCrossConeLayoutSections<'input> =
    LirConstituentsValidatedCrossConeLayoutSections<
        'input,
        lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::CompileProduction,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;
pub type LirDependenciesResolvedCrossConeLayoutClosure<'input> =
    LirConstituentsValidatedCrossConeLayoutClosure<
        'input,
        lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
        lir::CrossConeLirBridgeSectionV1,
        lir::CompileProduction,
        mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    >;

impl<'input> MirDependenciesResolvedCrossConeLayoutClosure<'input> {
    pub fn resolve_lir_dependencies(
        self,
    ) -> Result<
        LirDependenciesResolvedCrossConeLayoutClosure<'input>,
        CrossConeLayoutLirDependenciesError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let mut complete: Vec<LirDependenciesResolvedCrossConeLayoutSections<'input>> = Vec::new();
        for artifact in dependency_first {
            let provider = artifact.identity();
            let resolve = || -> Result<_, LirDependencyReadError> {
                let MirDependenciesResolvedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let layout = layout.resolve_dependencies(parts.identities)?;
                let path = WirePath::root();
                scoop_wire::allocation::try_reserve(&mut complete, 1, &path)?;
                Ok(LirDependenciesResolvedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                })
            };
            let artifact = resolve().map_err(|source| CrossConeLayoutLirDependenciesError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
        }
        Ok(LirDependenciesResolvedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: complete,
            positions,
            dependency_positions,
        })
    }
}

impl LirDependenciesResolvedCrossConeLayoutSections<'_> {
    pub const fn lir_dependency_transport(
        &self,
    ) -> &lir::DependencyResolvedCrossConeLayoutAbiSectionV1 {
        &self.layout
    }

    pub const fn lir_exports(&self) -> &lir::LayoutAbiExportConstituentsV1 {
        self.layout.exports()
    }

    pub const fn lir_production_data(&self) -> &lir::CompileProduction {
        &self.strong
    }
}
