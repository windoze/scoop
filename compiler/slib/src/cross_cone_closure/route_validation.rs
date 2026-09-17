//! Closure-wide public binding route validation.

use std::fmt;
use std::sync::Arc;

use scoop_hir::{
    CanonicalPublicExportBindingsV1, CrossConeHirInterfaceSectionV1, ExportBindingSourceV1,
    PublicExportBindingClosureAuthority, PublicExportBindingClosureValidationError,
};
use scoop_identity::{
    ConeIdentity, ExportBindingKey, PersistentExportBindingId, ValidatedIdentityGraph,
};
use scoop_lir::ValidatedLirTargetSelection;

use super::{
    ConstValidatedCrossConeHirClosure, CrossConeProviderRole,
    surface_validation::transitive_dependency_positions,
};
use crate::ConstValidatedCrossConeHirFrontSections;

mod authority_inputs;
mod external_target;

pub(super) use authority_inputs::{RouteAuthorityInputs, RouteProviderView};
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
}

impl<'input> ConstValidatedCrossConeHirClosure<'input> {
    /// Validates every provider route using only that provider's own direct
    /// edges and transitive support closure.
    pub fn validate_public_binding_routes(
        self,
    ) -> Result<PublicRouteValidatedCrossConeHirClosure<'input>, CrossConeClosurePublicRouteError>
    {
        {
            let (artifacts, dependency_positions) = self.route_validation_parts();
            for (position, artifact) in artifacts.iter().enumerate() {
                let identity = artifact.identity();
                let reachable = transitive_dependency_positions(position, dependency_positions);
                let inputs = RouteAuthorityInputs::try_new(
                    &artifacts[..position],
                    &dependency_positions[position],
                    &reachable,
                )
                .map_err(|requested_slots| {
                    CrossConeClosurePublicRouteError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;

                let authority = CanonicalCrossConeRouteAuthority::try_new(
                    identity,
                    artifact.identity_graph(),
                    artifact.hir_interface(),
                    inputs.direct(),
                    inputs.providers(),
                    inputs.closure_node_count(),
                )
                .map_err(|requested_slots| {
                    CrossConeClosurePublicRouteError::AuthorityAllocation {
                        identity,
                        requested_slots,
                    }
                })?;
                artifact
                    .hir_interface()
                    .public_bindings()
                    .validate_route_closure(identity, &authority)
                    .map_err(|source| CrossConeClosurePublicRouteError::Artifact {
                        identity,
                        source: Box::new(source),
                    })?;
            }
        }

        Ok(PublicRouteValidatedCrossConeHirClosure { surfaces: self })
    }
}

pub(super) struct CanonicalCrossConeRouteAuthority<'a> {
    current: ConeIdentity,
    identities: &'a ValidatedIdentityGraph,
    direct: &'a [ConeIdentity],
    providers: &'a [RouteProviderView<'a>],
    closure_node_count: usize,
    binding_keys: Vec<(PersistentExportBindingId, Arc<ExportBindingKey>)>,
}

impl<'a> CanonicalCrossConeRouteAuthority<'a> {
    pub(super) fn try_new(
        current: ConeIdentity,
        identities: &'a ValidatedIdentityGraph,
        interface: &CrossConeHirInterfaceSectionV1,
        direct: &'a [ConeIdentity],
        providers: &'a [RouteProviderView<'a>],
        closure_node_count: usize,
    ) -> Result<Self, usize> {
        let current_bindings = interface.public_bindings();
        let mut ids = Vec::new();
        ids.try_reserve_exact(current_bindings.records().len())
            .map_err(|_| current_bindings.records().len())?;
        for record in current_bindings.records() {
            ids.push(record.binding());
            if let ExportBindingSourceV1::Reexport { routes } = record.source() {
                for route in routes.routes() {
                    ids.try_reserve(route.hops().len())
                        .map_err(|_| ids.len().saturating_add(route.hops().len()))?;
                    ids.extend(route.hops().iter().map(|hop| hop.binding()));
                }
            }
        }
        for reference in interface.external_references().records() {
            for witness in reference.witnesses().witnesses() {
                ids.try_reserve(witness.route().hops().len())
                    .map_err(|_| ids.len().saturating_add(witness.route().hops().len()))?;
                ids.extend(witness.route().hops().iter().map(|hop| hop.binding()));
            }
        }
        ids.sort_unstable();
        ids.dedup();

        let mut binding_keys = Vec::new();
        binding_keys
            .try_reserve_exact(ids.len())
            .map_err(|_| ids.len())?;
        binding_keys.extend(ids.into_iter().filter_map(|binding| {
            identities
                .canonical_key::<PersistentExportBindingId, ExportBindingKey>(binding)
                .ok()
                .map(|key| (binding, key))
        }));

        Ok(Self {
            current,
            identities,
            direct,
            providers,
            closure_node_count,
            binding_keys,
        })
    }
}

impl PublicExportBindingClosureAuthority for CanonicalCrossConeRouteAuthority<'_> {
    fn closure_node_count(&self) -> usize {
        self.closure_node_count
    }

    fn is_direct_dependency(&self, provider: ConeIdentity) -> bool {
        self.direct.contains(&provider)
    }

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        self.binding_keys
            .binary_search_by_key(&binding, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.binding_keys[index].1.as_ref())
    }

    fn public_bindings(&self, exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        self.providers
            .iter()
            .find(|provider| provider.identity == exporter)
            .map(|provider| provider.bindings)
    }
}

#[derive(Debug)]
pub enum CrossConeClosurePublicRouteError {
    AuthorityAllocation {
        identity: ConeIdentity,
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<PublicExportBindingClosureValidationError>,
    },
}

impl fmt::Display for CrossConeClosurePublicRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorityAllocation {
                identity,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} public-route authority slots for {identity}"
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
            Self::AuthorityAllocation { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
