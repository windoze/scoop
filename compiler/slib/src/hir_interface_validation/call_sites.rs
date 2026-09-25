//! Call origins are resolved in the actual current/dependency foundations.

use scoop_hir::concrete::ExecutableExpressionPosition;
use scoop_hir::{
    DefinitionSourceLocationValidationError, ExecutableEvaluationValidationError,
    HirDependencyCallReasonV1, HirDependencyCallSignatureError, SharedTypeMetadataV1,
};
use scoop_wire::{WireError, WirePath};

use super::*;

mod runtime;
pub use runtime::CrossConeHirRuntimeCallError;

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
                self.executable_origin(site.position(), site.origin(), dependencies, meter, &path)?;
                match site.reason() {
                    HirDependencyCallReasonV1::SourceBinding(_) => {
                        meter.charge_work(dependencies.len() as u64 + 1, &path)?;
                        let provider = dependencies
                            .iter()
                            .find(|provider| provider.identity == reference.origin())
                            .ok_or(CrossConeHirCallSiteOriginError::UnreachableTarget {
                                position: site.position(),
                                provider: reference.origin(),
                            })?;
                        site.validate_source_signature(
                            reference.target(),
                            SharedTypeMetadataV1 {
                                provider: provider.identity,
                                identities: provider.identities,
                                foundation: provider.foundation,
                                public: provider.interface,
                            },
                            meter,
                            &path,
                        )
                        .map_err(|source| {
                            CrossConeHirCallSiteOriginError::Signature {
                                position: site.position(),
                                source: Box::new(source),
                            }
                        })?;
                    }
                    HirDependencyCallReasonV1::CastFailure { .. } => {
                        self.runtime_call(reference, site, dependencies, meter, &path)
                            .map_err(|source| CrossConeHirCallSiteOriginError::Runtime {
                                position: site.position(),
                                source: Box::new(source),
                            })?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn executable_origin(
        self,
        position: ExecutableExpressionPosition,
        origin: &scoop_identity::ConcreteExpressionOrigin,
        dependencies: &[ValidatedNominalProviderView<'_>],
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), CrossConeHirCallSiteOriginError> {
        let definition = origin.definition();
        let provider = definition.source().cone();
        meter.charge_work(dependencies.len() as u64 + 1, path)?;
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
            .validate_definition_origin_location(provider, definition, meter, path)
            .map_err(|source| CrossConeHirCallSiteOriginError::Definition {
                position,
                source: Box::new(source),
            })?;
        self.foundation
            .validate_executable_evaluation_origin(
                self.current,
                position.root,
                origin.evaluation(),
                meter,
                path,
            )
            .map_err(|source| CrossConeHirCallSiteOriginError::Evaluation {
                position,
                source: Box::new(source),
            })?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum CrossConeHirCallSiteOriginError {
    Resource(WireError),
    Signature {
        position: ExecutableExpressionPosition,
        source: Box<HirDependencyCallSignatureError>,
    },
    UnreachableTarget {
        position: ExecutableExpressionPosition,
        provider: ConeIdentity,
    },
    Runtime {
        position: ExecutableExpressionPosition,
        source: Box<CrossConeHirRuntimeCallError>,
    },
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
            Self::Signature { position, source } => {
                write!(f, "invalid source call signature at {position:?}: {source}")
            }
            Self::UnreachableTarget { position, provider } => {
                write!(
                    f,
                    "call {position:?} has an unreachable target provider {provider}"
                )
            }
            Self::Runtime { position, source } => {
                write!(f, "invalid runtime call at {position:?}: {source}")
            }
            Self::UnreachableDefinition { position, provider } => write!(
                f,
                "expression {position:?} has an unreachable definition provider {provider}"
            ),
            Self::Definition { position, source } => {
                write!(f, "invalid definition of expression {position:?}: {source}")
            }
            Self::Evaluation { position, source } => {
                write!(f, "invalid evaluation of expression {position:?}: {source}")
            }
        }
    }
}

impl std::error::Error for CrossConeHirCallSiteOriginError {}
