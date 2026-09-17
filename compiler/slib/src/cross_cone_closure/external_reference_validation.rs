//! Closure-wide external HIR reference validation.

use std::fmt;

use scoop_hir::CrossConeHirExternalReferenceValidationError;
use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::WirePath;

use super::{
    CrossConeProviderRole, PublicRouteValidatedCrossConeHirClosure,
    route_validation::{
        CanonicalCrossConeRouteAuthority, CrossConeHirReferenceAuthorityError, RouteAuthorityInputs,
    },
    surface_validation::transitive_dependency_positions,
};
use crate::{ConstValidatedCrossConeHirClosure, ConstValidatedCrossConeHirFrontSections};

/// A route-validated closure whose complete export-derived external HIR
/// reference set has been reconstructed and checked artifact by artifact.
pub struct ExternalReferenceValidatedCrossConeHirClosure<'input> {
    routes: PublicRouteValidatedCrossConeHirClosure<'input>,
}

impl<'input> ExternalReferenceValidatedCrossConeHirClosure<'input> {
    pub const fn current(&self) -> ConeIdentity {
        self.routes.current()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.routes.target_selection()
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        self.routes.direct_providers()
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &ConstValidatedCrossConeHirFrontSections<'_>> {
        self.routes.dependency_first()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ConstValidatedCrossConeHirFrontSections<'_>> {
        self.routes.artifact(identity)
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.routes.role(identity)
    }

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.routes.dependency_count(identity)
    }

    pub(super) fn surfaces_mut(&mut self) -> &mut ConstValidatedCrossConeHirClosure<'input> {
        self.routes.surfaces_mut()
    }
}

impl<'input> PublicRouteValidatedCrossConeHirClosure<'input> {
    /// Validates record semantics, route witnesses, and the exact re-export,
    /// signature, alias, default-dependency, and const-type closures.
    pub fn validate_external_hir_references(
        mut self,
    ) -> Result<
        ExternalReferenceValidatedCrossConeHirClosure<'input>,
        CrossConeClosureExternalReferenceError,
    > {
        {
            let (artifacts, dependency_positions) =
                self.surfaces_mut().hir_semantic_validation_parts();
            for position in 0..artifacts.len() {
                let reachable = transitive_dependency_positions(position, dependency_positions);
                let (previous, current_and_later) = artifacts.split_at_mut(position);
                let current = &mut current_and_later[0];
                let identity = current.identity();
                let route_inputs = RouteAuthorityInputs::try_new(
                    previous,
                    &dependency_positions[position],
                    &reachable,
                )
                .map_err(|requested_slots| {
                    CrossConeClosureExternalReferenceError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
                let (identities, interface, meter) = current.hir_semantic_parts();
                let mut authority = CanonicalCrossConeRouteAuthority::try_new(
                    identity,
                    identities,
                    interface,
                    route_inputs.direct(),
                    route_inputs.providers(),
                    route_inputs.closure_node_count(),
                )
                .map_err(|requested_slots| {
                    CrossConeClosureExternalReferenceError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
                interface
                    .validate_external_reference_closure(&mut authority, meter, &WirePath::root())
                    .map_err(|source| CrossConeClosureExternalReferenceError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?;
            }
        }

        Ok(ExternalReferenceValidatedCrossConeHirClosure { routes: self })
    }
}

#[derive(Debug)]
pub enum CrossConeClosureExternalReferenceError {
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source:
            Box<CrossConeHirExternalReferenceValidationError<CrossConeHirReferenceAuthorityError>>,
    },
}

impl fmt::Display for CrossConeClosureExternalReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} external-reference authority slots for {identity}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid external HIR reference closure for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureExternalReferenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::AuthorityAllocation { .. } => None,
        }
    }
}
