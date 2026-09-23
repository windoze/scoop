//! Per-artifact exported definition-source validation.

use scoop_hir::{
    ExportDefinitionSourceSemanticValidationError,
    ExportDefinitionSourceSetSemanticValidationError, OdrFreeHirFoundation,
};
use scoop_identity::ConeIdentity;
use scoop_wire::{WireError, WirePath};

use super::{InternallyClosedCrossConeHirFrontSections, ValidatedSurfaceFront};
use crate::cross_cone_hir_authority::CrossConeHirDefinitionSourceAuthorityError;

/// Only foundation location facts; this view grants no nominal or callable access.
#[derive(Clone, Copy)]
pub(crate) struct DefinitionSourceProviderView<'a> {
    pub(crate) identity: ConeIdentity,
    pub(crate) foundation: &'a OdrFreeHirFoundation,
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
        let current = graph.identity();
        let meter = graph.envelope.meter_mut();
        let path = WirePath::root().field(9);
        let sources = hir_interface.definition_sources().sources();
        meter
            .check_table_entries(sources.len() as u64, &path)
            .map_err(CrossConeHirDefinitionSourceSurfaceError::Resource)?;
        for (index, source) in sources.iter().enumerate() {
            let path = path.clone().index(index as u64);
            let provider = source.origin().source().cone();
            meter
                .charge_work(
                    (dependencies.len() as u64)
                        .saturating_mul(32)
                        .saturating_add(1),
                    &path,
                )
                .map_err(CrossConeHirDefinitionSourceSurfaceError::Resource)?;
            let foundation = if provider == current {
                &foundations.hir
            } else {
                dependencies
                    .iter()
                    .find(|dependency| dependency.identity == provider)
                    .map(|dependency| dependency.foundation)
                    .ok_or(
                        CrossConeHirDefinitionSourceSurfaceError::UnavailableProvider {
                            index,
                            provider,
                        },
                    )?
            };
            foundation
                .validate_definition_source_location(provider, source, meter, &path)
                .map_err(|error| {
                    CrossConeHirDefinitionSourceSurfaceError::DefinitionSources(
                        ExportDefinitionSourceSetSemanticValidationError::Source {
                            index,
                            error: ExportDefinitionSourceSemanticValidationError::Foundation(error),
                        },
                    )
                })?;
        }
        Ok(DefinitionSourceValidatedCrossConeHirFrontSections(
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
