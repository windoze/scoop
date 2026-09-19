//! HIR transport and unchanged bootstrap-production closure transitions.

use std::fmt;

use scoop_identity::ConeIdentity;

use super::*;
use crate::{CrossConeClosureHirProductionError, CrossConeLayoutHirResolutionError};

impl<'input> FoundationValidatedCrossConeLayoutCompileClosure<'input> {
    /// Resolves both HIR transports in every artifact without changing graph
    /// order or allowing one provider's identity graph to resolve another.
    pub fn resolve_hir_sections(
        self,
    ) -> Result<ResolvedCrossConeLayoutHirClosure<'input>, CrossConeLayoutClosureHirResolutionError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut resolved = Vec::new();
        resolved.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeLayoutClosureHirResolutionError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for artifact in dependency_first {
            let identity = artifact.identity();
            resolved.push(artifact.resolve_hir_sections().map_err(|source| {
                CrossConeLayoutClosureHirResolutionError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?);
        }
        Ok(ResolvedCrossConeLayoutHirClosure {
            current,
            target,
            direct,
            dependency_first: resolved,
            positions,
            dependency_positions,
        })
    }
}

impl<'input> ResolvedCrossConeLayoutHirClosure<'input> {
    /// Replays each provider's unchanged core-bootstrap HIR contract before
    /// the public and type-semantics tables can enter closure validation.
    pub fn validate_hir_productions(
        self,
    ) -> Result<
        HirProductionValidatedCrossConeLayoutClosure<'input>,
        CrossConeClosureHirProductionError,
    > {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureHirProductionError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for artifact in dependency_first {
            let identity = artifact.identity();
            validated.push(artifact.validate_hir_production().map_err(|source| {
                CrossConeClosureHirProductionError::Artifact { identity, source }
            })?);
        }
        Ok(HirProductionValidatedCrossConeLayoutClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}

#[derive(Debug)]
pub enum CrossConeLayoutClosureHirResolutionError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeLayoutHirResolutionError>,
    },
}

impl fmt::Display for CrossConeLayoutClosureHirResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} resolved layout-profile HIR slots"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "cannot resolve layout-profile HIR for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeLayoutClosureHirResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } => None,
        }
    }
}
