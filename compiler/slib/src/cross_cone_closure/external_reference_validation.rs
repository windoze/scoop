//! Closure-wide external HIR reference validation.

use std::fmt;

use scoop_hir::CrossConeHirExternalReferenceValidationError;
use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{WireError, WirePath};

use super::{
    CrossConeProviderRole, PublicRouteValidatedCrossConeHirClosure,
    route_validation::{
        CanonicalCrossConeRouteAuthority, CrossConeHirReferenceAuthorityError, RouteAuthorityInputs,
    },
};
use crate::{
    ConstValidatedCrossConeHirClosure, ConstValidatedCrossConeHirFrontSections,
    CrossConeHirCallSiteOriginError,
};

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

    pub(super) fn into_routes(self) -> PublicRouteValidatedCrossConeHirClosure<'input> {
        self.routes
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
                let (previous, current_and_later) = artifacts.split_at_mut(position);
                let current = &mut current_and_later[0];
                let identity = current.identity();
                let input = current.hir_reference_validation_parts();
                let identities = input.identities;
                let interface = input.interface;
                let path = WirePath::root();
                let resource =
                    |source| CrossConeClosureExternalReferenceError::Resource { identity, source };
                let reachable = crate::dependency_reachability::transitive_positions(
                    position,
                    dependency_positions,
                )
                .map_err(resource)?;
                let route_inputs = RouteAuthorityInputs::try_new(
                    previous,
                    &dependency_positions[position],
                    &reachable,
                    &path,
                )
                .map_err(resource)?;
                let mut authority = CanonicalCrossConeRouteAuthority::try_new(
                    identity,
                    identities,
                    interface,
                    route_inputs.direct(),
                    route_inputs.providers(),
                    &path,
                )
                .map_err(resource)?;
                interface
                    .validate_external_reference_closure(&mut authority, &WirePath::root())
                    .map_err(|source| CrossConeClosureExternalReferenceError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?;
                let mut providers = Vec::new();
                scoop_wire::allocation::try_reserve(
                    &mut providers,
                    reachable.len(),
                    &WirePath::root(),
                )
                .map_err(|source| {
                    CrossConeClosureExternalReferenceError::CallSites {
                        identity,
                        source: Box::new(CrossConeHirCallSiteOriginError::Resource(source)),
                    }
                })?;
                providers.extend(
                    reachable
                        .iter()
                        .map(|&index| previous[index].nominal_provider_view()),
                );
                input.call_sites(&providers).map_err(|source| {
                    CrossConeClosureExternalReferenceError::CallSites {
                        identity,
                        source: Box::new(source),
                    }
                })?;
                input.type_sites(&providers).map_err(|source| {
                    CrossConeClosureExternalReferenceError::TypeSites {
                        identity,
                        source: Box::new(source),
                    }
                })?;
            }
        }

        Ok(ExternalReferenceValidatedCrossConeHirClosure { routes: self })
    }
}

#[derive(Debug)]
pub enum CrossConeClosureExternalReferenceError {
    TypeSites {
        identity: ConeIdentity,
        source: Box<crate::hir_interface_validation::CrossConeHirTypeSiteError>,
    },
    CallSites {
        identity: ConeIdentity,
        source: Box<CrossConeHirCallSiteOriginError>,
    },
    Resource {
        identity: ConeIdentity,
        source: WireError,
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
            Self::TypeSites { identity, source } => {
                write!(formatter, "invalid HIR type sites for {identity}: {source}")
            }
            Self::CallSites { identity, source } => {
                write!(formatter, "invalid HIR call sites for {identity}: {source}")
            }
            Self::Resource { identity, source } => write!(
                formatter,
                "external-reference resources exhausted for {identity}: {source}"
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
            Self::TypeSites { source, .. } => Some(source.as_ref()),
            Self::CallSites { source, .. } => Some(source.as_ref()),
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Resource { source, .. } => Some(source),
        }
    }
}
