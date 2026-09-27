//! Per-artifact exported definition-source validation.

use scoop_hir::ExportDefinitionSourceSetSemanticValidationError;
use scoop_identity::ConeIdentity;
use scoop_wire::WireError;

use super::{InternallyClosedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::CrossConeHirDefinitionSourceAuthorityError;

/// Only foundation location facts; this view grants no nominal or callable access.
#[derive(Clone, Copy)]
pub(crate) struct DefinitionSourceProviderView<'a> {
    pub(crate) identity: ConeIdentity,
    pub(crate) foundation: &'a scoop_hir::CanonicalHirFoundation,
}

/// One provider whose inline locations are backed by the exact foundation of
/// their actual source provider, reached through this artifact's dependencies.
pub struct DefinitionSourceValidatedCrossConeHirFrontSections<'input>(
    pub(super) ValidatedSurfaceFront<'input>,
);

impl DefinitionSourceValidatedCrossConeHirFrontSections<'_> {
    pub(crate) fn definition_source_provider_view(&self) -> DefinitionSourceProviderView<'_> {
        DefinitionSourceProviderView {
            identity: self.0.graph.identity(),
            foundation: &self.0.foundations.hir,
        }
    }
}

impl<'input> InternallyClosedCrossConeHirFrontSections<'input> {
    pub(crate) fn validate_definition_sources(
        self,
        dependencies: &[DefinitionSourceProviderView<'_>],
    ) -> Result<
        DefinitionSourceValidatedCrossConeHirFrontSections<'input>,
        CrossConeHirDefinitionSourceSurfaceError,
    > {
        let mut front = self.0;
        let input = front.hir_validation_parts();
        input.definition_sources(dependencies)?;
        Ok(DefinitionSourceValidatedCrossConeHirFrontSections(front))
    }
}

#[derive(Debug)]
pub enum CrossConeHirDefinitionSourceSurfaceError {
    Resource(WireError),
    UnavailableProvider {
        index: usize,
        provider: ConeIdentity,
    },
    DefinitionSources(
        ExportDefinitionSourceSetSemanticValidationError<
            CrossConeHirDefinitionSourceAuthorityError,
        >,
    ),
}

impl std::fmt::Display for CrossConeHirDefinitionSourceSurfaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::UnavailableProvider { index, provider } => write!(
                formatter,
                "definition source[{index}] provider {provider} is not reachable from this artifact"
            ),
            Self::DefinitionSources(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirDefinitionSourceSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::UnavailableProvider { .. } => None,
            Self::DefinitionSources(error) => Some(error),
        }
    }
}
