//! Closure-wide public binding route validation.

use std::fmt;
use std::sync::Arc;

use scoop_hir::PublicExportBindingClosureValidationError;
use scoop_identity::{
    ConeIdentity, ExportBindingKey, PersistentExportBindingId, ValidatedIdentityGraph,
};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{WireError, WirePath};

use super::{ConstValidatedCrossConeHirClosure, CrossConeProviderRole};
use crate::ConstValidatedCrossConeHirFrontSections;

mod authority;
mod authority_inputs;
mod external_target;
mod index;

pub(super) use authority_inputs::RouteAuthorityInputs;
pub(crate) use authority_inputs::{RouteProviderView, reserve_route_slots};
pub use external_target::CrossConeHirReferenceAuthorityError;

/// A closure whose public binding routes have been checked against each
/// provider's exact direct and transitive dependency graph.
pub struct PublicRouteValidatedCrossConeHirClosure<'input> {
    surfaces: ConstValidatedCrossConeHirClosure<'input>,
}

impl<'input> PublicRouteValidatedCrossConeHirClosure<'input> {
    pub const fn current(&self) -> ConeIdentity {
        self.surfaces.current()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.surfaces.target_selection()
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        self.surfaces.direct_providers()
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &ConstValidatedCrossConeHirFrontSections<'_>> {
        self.surfaces.dependency_first()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ConstValidatedCrossConeHirFrontSections<'_>> {
        self.surfaces.artifact(identity)
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.surfaces.role(identity)
    }

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.surfaces.dependency_count(identity)
    }

    pub(super) fn surfaces_mut(&mut self) -> &mut ConstValidatedCrossConeHirClosure<'input> {
        &mut self.surfaces
    }

    pub(super) fn into_surfaces(self) -> ConstValidatedCrossConeHirClosure<'input> {
        self.surfaces
    }
}

impl<'input> ConstValidatedCrossConeHirClosure<'input> {
    /// Validates every provider route against its own direct and transitive
    /// dependencies.
    pub fn validate_public_binding_routes(
        mut self,
    ) -> Result<PublicRouteValidatedCrossConeHirClosure<'input>, CrossConeClosurePublicRouteError>
    {
        let (artifacts, dependency_positions) = self.hir_semantic_validation_parts();
        for position in 0..artifacts.len() {
            let (previous, current_and_later) = artifacts.split_at_mut(position);
            let current = &mut current_and_later[0];
            let identity = current.identity();
            let (identities, interface) = current.hir_semantic_parts();
            let path = WirePath::root();
            let resource = |source| CrossConeClosurePublicRouteError::Resource { identity, source };
            let reachable = crate::dependency_reachability::transitive_positions(
                position,
                dependency_positions,
            )
            .map_err(resource)?;
            let inputs = RouteAuthorityInputs::try_new(
                previous,
                &dependency_positions[position],
                &reachable,
                &path,
            )
            .map_err(resource)?;
            let authority = CanonicalCrossConeRouteAuthority::try_new(
                identity,
                identities,
                interface,
                inputs.direct(),
                inputs.providers(),
                &path,
            )
            .map_err(resource)?;
            interface
                .public_bindings()
                .validate_route_closure(identity, &authority)
                .map_err(|source| CrossConeClosurePublicRouteError::Artifact {
                    identity,
                    source: Box::new(source),
                })?;
        }
        Ok(PublicRouteValidatedCrossConeHirClosure { surfaces: self })
    }
}

pub(crate) struct CanonicalCrossConeRouteAuthority<'a> {
    current: ConeIdentity,
    identities: &'a ValidatedIdentityGraph,
    direct: &'a [ConeIdentity],
    providers: &'a [RouteProviderView<'a>],
    closure_node_count: usize,
    binding_keys: Vec<(PersistentExportBindingId, Arc<ExportBindingKey>)>,
}

#[derive(Debug)]
pub enum CrossConeClosurePublicRouteError {
    Resource {
        identity: ConeIdentity,
        source: WireError,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<PublicExportBindingClosureValidationError>,
    },
}

impl fmt::Display for CrossConeClosurePublicRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource { identity, source } => write!(
                formatter,
                "public binding route resources exhausted for {identity}: {source}"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid public binding routes for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosurePublicRouteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Resource { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests;
