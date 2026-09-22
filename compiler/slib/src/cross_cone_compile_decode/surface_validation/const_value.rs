//! Per-artifact exported constant validation.

use scoop_hir::ExportConstValueSetSemanticValidationError;

use super::{SourceInterfaceValidatedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirConstAuthorityError,
    ValidatedNominalProviderView,
};

/// One provider whose portable const records match its validated property
/// surface and the shared intrinsic declarations of their actual types.
pub struct ConstValidatedCrossConeHirFrontSections<'input>(
    pub(super) ValidatedSurfaceFront<'input>,
);

impl<'input> SourceInterfaceValidatedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_const_values<'dependency>(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'dependency>>,
    ) -> Result<ConstValidatedCrossConeHirFrontSections<'input>, CrossConeHirConstSurfaceError>
    {
        let ValidatedSurfaceFront {
            mut graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            mir_cross_cone_bridge,
            lir_strong_production,
            lir_cross_cone_bridge,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_interface,
            dependencies,
            graph.envelope.meter_mut(),
        );
        hir_interface
            .constants()
            .validate_semantics(&mut authority)
            .map_err(CrossConeHirConstSurfaceError::Constants)?;
        Ok(ConstValidatedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                mir_cross_cone_bridge,
                lir_strong_production,
                lir_cross_cone_bridge,
            },
        ))
    }
}

#[derive(Debug)]
pub enum CrossConeHirConstSurfaceError {
    Constants(ExportConstValueSetSemanticValidationError<CrossConeHirConstAuthorityError>),
}

impl std::fmt::Display for CrossConeHirConstSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Constants(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirConstSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Constants(error) => Some(error),
        }
    }
}
