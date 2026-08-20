//! Closure-wide exported constant validation.

use std::fmt;

use scoop_identity::ConeIdentity;

use super::{
    SourceInterfaceValidatedCrossConeHirClosure, ValidatedSurfaceClosure, nominal_dependencies,
};
use crate::{ConstValidatedCrossConeHirFrontSections, CrossConeHirConstSurfaceError};

/// Providers whose portable const records are tied to exact property and
/// trusted-core type authority.
pub struct ConstValidatedCrossConeHirClosure<'input>(
    pub(super) ValidatedSurfaceClosure<ConstValidatedCrossConeHirFrontSections<'input>>,
);

impl<'input> SourceInterfaceValidatedCrossConeHirClosure<'input> {
    pub fn validate_const_values(
        self,
    ) -> Result<ConstValidatedCrossConeHirClosure<'input>, CrossConeClosureConstError> {
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
            CrossConeClosureConstError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let dependencies = nominal_dependencies(position, &dependency_positions, &validated)
                .map_err(
                    |requested_slots| CrossConeClosureConstError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    },
                )?;
            validated.push(
                front
                    .validate_const_values(dependencies)
                    .map_err(|source| CrossConeClosureConstError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?,
            );
        }
        Ok(ConstValidatedCrossConeHirClosure(ValidatedSurfaceClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        }))
    }
}

#[derive(Debug)]
pub enum CrossConeClosureConstError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirConstSurfaceError>,
    },
}

impl fmt::Display for CrossConeClosureConstError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} const-validated cross-Cone HIR slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} constant authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid exported constants for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureConstError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}
