//! Closure-wide callable source-interface validation.

use std::fmt;

use scoop_identity::ConeIdentity;

use super::{TypeAliasValidatedCrossConeHirClosure, ValidatedSurfaceClosure, nominal_dependencies};
use crate::{
    CrossConeHirSourceInterfaceSurfaceError, SourceInterfaceValidatedCrossConeHirFrontSections,
};

/// Providers whose source-order callable parameter metadata is canonical.
pub struct SourceInterfaceValidatedCrossConeHirClosure<'input>(
    pub(super) ValidatedSurfaceClosure<SourceInterfaceValidatedCrossConeHirFrontSections<'input>>,
);

impl<'input> TypeAliasValidatedCrossConeHirClosure<'input> {
    pub fn validate_source_interfaces(
        self,
    ) -> Result<
        SourceInterfaceValidatedCrossConeHirClosure<'input>,
        CrossConeClosureSourceInterfaceError,
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
            CrossConeClosureSourceInterfaceError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for (position, front) in dependency_first.into_iter().enumerate() {
            let identity = front.identity();
            let dependencies = nominal_dependencies(position, &dependency_positions, &validated)
                .map_err(|requested_slots| {
                    CrossConeClosureSourceInterfaceError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
            validated.push(
                front
                    .validate_source_interfaces(dependencies)
                    .map_err(|source| CrossConeClosureSourceInterfaceError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?,
            );
        }
        Ok(SourceInterfaceValidatedCrossConeHirClosure(
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
pub enum CrossConeClosureSourceInterfaceError {
    Allocation {
        requested_slots: usize,
    },
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirSourceInterfaceSurfaceError>,
    },
}

impl fmt::Display for CrossConeClosureSourceInterfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} source-interface-validated cross-Cone HIR slots"
            ),
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} source-interface authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => write!(
                formatter,
                "invalid callable source interfaces for {identity}: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeClosureSourceInterfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
        }
    }
}
