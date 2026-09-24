//! Call origins are resolved in the actual current/dependency foundations.

use scoop_hir::concrete::ExecutableExpressionPosition;
use scoop_hir::{DefinitionSourceLocationValidationError, ExecutableEvaluationValidationError};
use scoop_wire::{WireError, WirePath};

use super::*;

impl HirInterfaceValidationInput<'_> {
    pub(crate) fn call_sites(
        self,
        dependencies: &[ValidatedNominalProviderView<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<(), CrossConeHirCallSiteOriginError> {
        let path = WirePath::root().field(10);
        for (index, reference) in self
            .interface
            .external_references()
            .records()
            .iter()
            .enumerate()
        {
            meter.charge_work(1, &path)?;
            for (site_index, site) in reference.call_sites().records().iter().enumerate() {
                let path = path
                    .clone()
                    .index(index as u64)
                    .field(5)
                    .index(site_index as u64);
                let position = site.position();
                let definition = site.origin().definition();
                let provider = definition.source().cone();
                meter.charge_work(dependencies.len() as u64 + 1, &path)?;
                let foundation = if provider == self.current {
                    self.foundation
                } else {
                    dependencies
                        .iter()
                        .find(|view| view.identity == provider)
                        .map(|view| view.foundation)
                        .ok_or(CrossConeHirCallSiteOriginError::UnreachableDefinition {
                            position,
                            provider,
                        })?
                };
                foundation
                    .validate_definition_origin_location(provider, definition, meter, &path)
                    .map_err(|source| CrossConeHirCallSiteOriginError::Definition {
                        position,
                        source: Box::new(source),
                    })?;
                self.foundation
                    .validate_executable_evaluation_origin(
                        self.current,
                        position.root,
                        site.origin().evaluation(),
                        meter,
                        &path,
                    )
                    .map_err(|source| CrossConeHirCallSiteOriginError::Evaluation {
                        position,
                        source: Box::new(source),
                    })?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirCallSiteOriginError {
    Resource(WireError),
    UnreachableDefinition {
        position: ExecutableExpressionPosition,
        provider: ConeIdentity,
    },
    Definition {
        position: ExecutableExpressionPosition,
        source: Box<DefinitionSourceLocationValidationError>,
    },
    Evaluation {
        position: ExecutableExpressionPosition,
        source: Box<ExecutableEvaluationValidationError>,
    },
}

impl From<WireError> for CrossConeHirCallSiteOriginError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for CrossConeHirCallSiteOriginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(f),
            Self::UnreachableDefinition { position, provider } => write!(
                f,
                "call {position:?} has an unreachable definition provider {provider}"
            ),
            Self::Definition { position, source } => {
                write!(f, "invalid definition of call {position:?}: {source}")
            }
            Self::Evaluation { position, source } => {
                write!(f, "invalid evaluation of call {position:?}: {source}")
            }
        }
    }
}

impl std::error::Error for CrossConeHirCallSiteOriginError {}
