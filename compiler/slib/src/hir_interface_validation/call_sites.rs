//! Call origins are resolved in the actual current/dependency foundations.

use scoop_hir::concrete::ExecutableExpressionPosition;
use scoop_hir::{
    DefinitionSourceLocationValidationError, ExecutableEvaluationValidationError,
    HirDependencyCallSignatureError, SharedTypeMetadataV1,
};
use scoop_wire::WireError;

use super::*;

impl HirInterfaceValidationInput<'_> {
    pub(crate) fn call_sites(
        self,
        dependencies: &[ValidatedNominalProviderView<'_>],
    ) -> Result<(), CrossConeHirCallSiteOriginError> {
        for reference in self.interface.external_references().records() {
            for site in reference.call_sites().records() {
                self.executable_origin(site.position(), site.origin(), dependencies)?;
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
                    self.identities,
                    |owner| {
                        let owner = scoop_hir::SourceNominalId::Concrete(owner);
                        self.interface.nominal_interfaces().get(owner).or_else(|| {
                            dependencies.iter().find_map(|provider| {
                                provider.interface.nominal_interfaces().get(owner)
                            })
                        })
                    },
                )
                .map_err(|source| CrossConeHirCallSiteOriginError::Signature {
                    position: site.position(),
                    source: Box::new(source),
                })?;
            }
        }
        Ok(())
    }

    pub(super) fn executable_origin(
        self,
        position: ExecutableExpressionPosition,
        origin: &scoop_identity::ConcreteExpressionOrigin,
        dependencies: &[ValidatedNominalProviderView<'_>],
    ) -> Result<(), CrossConeHirCallSiteOriginError> {
        let definition = origin.definition();
        let provider = definition.source().cone();

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
            .validate_definition_origin_location(provider, definition)
            .map_err(|source| CrossConeHirCallSiteOriginError::Definition {
                position,
                source: Box::new(source),
            })?;
        let evaluation_provider = origin.evaluation().source().cone();
        let evaluation_foundation = if evaluation_provider == self.current {
            self.foundation
        } else {
            dependencies
                .iter()
                .find(|view| view.identity == evaluation_provider)
                .map(|view| view.foundation)
                .ok_or(CrossConeHirCallSiteOriginError::UnreachableEvaluation {
                    position,
                    provider: evaluation_provider,
                })?
        };
        evaluation_foundation
            .validate_executable_evaluation_origin(
                evaluation_provider,
                position.root,
                origin.evaluation(),
                self.foundation,
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
    UnreachableDefinition {
        position: ExecutableExpressionPosition,
        provider: ConeIdentity,
    },
    UnreachableEvaluation {
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
            Self::UnreachableDefinition { position, provider } => write!(
                f,
                "expression {position:?} has an unreachable definition provider {provider}"
            ),
            Self::UnreachableEvaluation { position, provider } => write!(
                f,
                "expression {position:?} has an unreachable evaluation provider {provider}"
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
