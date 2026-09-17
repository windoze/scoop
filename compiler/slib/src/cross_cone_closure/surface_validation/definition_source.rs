//! Closure-wide exported definition-source validation.

use std::fmt;

use scoop_identity::ConeIdentity;

use super::{InternallyClosedCrossConeHirClosure, ValidatedSurfaceClosure};
use crate::{
    CrossConeHirDefinitionSourceSurfaceError, DefinitionSourceValidatedCrossConeHirFrontSections,
};

/// Providers whose exported definition locations are all backed by their own
/// validated HIR foundation records.
pub struct DefinitionSourceValidatedCrossConeHirClosure<'input>(
    pub(super) ValidatedSurfaceClosure<DefinitionSourceValidatedCrossConeHirFrontSections<'input>>,
);

impl<'input> InternallyClosedCrossConeHirClosure<'input> {
    pub fn validate_definition_sources(
        self,
    ) -> Result<
        DefinitionSourceValidatedCrossConeHirClosure<'input>,
        CrossConeClosureDefinitionSourceError,
    > {
        let ValidatedSurfaceClosure {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self.0;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureDefinitionSourceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            validated.push(front.validate_definition_sources().map_err(|source| {
                CrossConeClosureDefinitionSourceError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?);
        }
        Ok(DefinitionSourceValidatedCrossConeHirClosure(
            ValidatedSurfaceClosure {
                current,
                target,
                direct,
                dependency_first: validated,
                positions,
                dependency_positions,
            },
        ))
    }
}

#[derive(Debug)]
pub enum CrossConeClosureDefinitionSourceError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirDefinitionSourceSurfaceError>,
    },
}

impl fmt::Display for CrossConeClosureDefinitionSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} definition-source-validated cross-Cone HIR slots"
            ),
            Self::Artifact { identity, source } => write!(
                formatter,
                "invalid exported definition sources for {identity}: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeClosureDefinitionSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } => None,
        }
    }
}
