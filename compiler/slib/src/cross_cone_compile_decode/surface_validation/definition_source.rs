//! Per-artifact exported definition-source validation.

use scoop_hir::ExportDefinitionSourceSetSemanticValidationError;

use super::{InternallyClosedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirDefinitionSourceAuthorityError,
};

/// One provider whose exported definition locations are backed by its exact
/// HIR foundation source, context, and point records.
pub struct DefinitionSourceValidatedCrossConeHirFrontSections<'input>(
    pub(super) ValidatedSurfaceFront<'input>,
);

impl<'input> InternallyClosedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_definition_sources(
        self,
    ) -> Result<
        DefinitionSourceValidatedCrossConeHirFrontSections<'input>,
        CrossConeHirDefinitionSourceSurfaceError,
    > {
        let ValidatedSurfaceFront {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_core_production,
            &hir_interface,
            Vec::new(),
        );
        hir_interface
            .definition_sources()
            .validate_semantics(&mut authority)
            .map_err(CrossConeHirDefinitionSourceSurfaceError::DefinitionSources)?;
        Ok(DefinitionSourceValidatedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                lir_strong_production,
            },
        ))
    }
}

#[derive(Debug)]
pub enum CrossConeHirDefinitionSourceSurfaceError {
    DefinitionSources(
        ExportDefinitionSourceSetSemanticValidationError<
            CrossConeHirDefinitionSourceAuthorityError,
        >,
    ),
}

impl std::fmt::Display for CrossConeHirDefinitionSourceSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DefinitionSources(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirDefinitionSourceSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefinitionSources(error) => Some(error),
        }
    }
}
