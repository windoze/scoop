use std::fmt;

use scoop_identity::{
    BindableEntity, BindingNamespace, BindingRole, ConeIdentity, ExportBindingKey,
    PersistentExportBindingId,
};

use super::{CanonicalPublicExportBindingsV1, ExportBindingSourceV1};
use crate::{CanonicalReexportRoutesV1, ReexportRouteHopV1};

/// Read-only authority used to validate the route graph of one artifact.
///
/// Implementations are scoped to the artifact passed to
/// [`CanonicalPublicExportBindingsV1::validate_route_closure`], so direct
/// dependency membership is always relative to that current artifact.
pub trait PublicExportBindingClosureAuthority {
    fn closure_node_count(&self) -> usize;

    fn is_direct_dependency(&self, provider: ConeIdentity) -> bool;

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey>;

    fn public_bindings(&self, exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1>;
}

impl CanonicalPublicExportBindingsV1 {
    /// Validates every cross-Cone route against the complete artifact closure.
    ///
    /// Record decoding establishes typed identity authority and canonical
    /// ordering. This pass establishes the relational facts that cannot be
    /// checked from one record alone: direct-provider membership, invariant
    /// target/namespace/role, terminal declaration, and exact suffix
    /// continuity through every intermediate re-export.
    pub fn validate_route_closure<A>(
        &self,
        current: ConeIdentity,
        authority: &A,
    ) -> Result<(), PublicExportBindingClosureValidationError>
    where
        A: PublicExportBindingClosureAuthority,
    {
        for record in self.records() {
            let binding = record.binding();
            let key = authority.binding_key(binding).ok_or(
                PublicExportBindingClosureValidationError::MissingCurrentBindingKey { binding },
            )?;
            if key.exporter() != current {
                return Err(
                    PublicExportBindingClosureValidationError::CurrentBindingExporterMismatch {
                        binding,
                        expected: current,
                        actual: key.exporter(),
                    },
                );
            }

            match record.source() {
                ExportBindingSourceV1::DeclaredCurrent { declaration } => {
                    require_declared_target(current, binding, key, *declaration)?;
                }
                ExportBindingSourceV1::Reexport { routes } => {
                    validate_routes(binding, key, routes, authority)?;
                }
            }
        }
        Ok(())
    }
}

fn validate_routes<A>(
    binding: PersistentExportBindingId,
    binding_key: &ExportBindingKey,
    routes: &CanonicalReexportRoutesV1,
    authority: &A,
) -> Result<(), PublicExportBindingClosureValidationError>
where
    A: PublicExportBindingClosureAuthority,
{
    for (route_index, route) in routes.routes().iter().enumerate() {
        if !authority.is_direct_dependency(route.immediate_provider()) {
            return Err(
                PublicExportBindingClosureValidationError::ImmediateProviderNotDirect {
                    binding,
                    route: route_index,
                    provider: route.immediate_provider(),
                },
            );
        }
        if route.hops().len() > authority.closure_node_count() {
            return Err(
                PublicExportBindingClosureValidationError::RouteExceedsClosure {
                    binding,
                    route: route_index,
                    hops: route.hops().len(),
                    closure_nodes: authority.closure_node_count(),
                },
            );
        }

        for (hop_index, hop) in route.hops().iter().copied().enumerate() {
            validate_hop(
                binding,
                binding_key,
                route_index,
                hop_index,
                route.hops(),
                hop,
                authority,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_hop<A>(
    binding: PersistentExportBindingId,
    binding_key: &ExportBindingKey,
    route_index: usize,
    hop_index: usize,
    route_hops: &[ReexportRouteHopV1],
    hop: ReexportRouteHopV1,
    authority: &A,
) -> Result<(), PublicExportBindingClosureValidationError>
where
    A: PublicExportBindingClosureAuthority,
{
    let hop_key = authority.binding_key(hop.binding()).ok_or(
        PublicExportBindingClosureValidationError::MissingHopBindingKey {
            binding,
            route: route_index,
            hop: hop_index,
            hop_binding: hop.binding(),
        },
    )?;
    if hop_key.exporter() != hop.exporter() {
        return Err(
            PublicExportBindingClosureValidationError::HopBindingExporterMismatch {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
                expected: hop.exporter(),
                actual: Box::new(hop_key.exporter()),
            },
        );
    }
    if hop_key.target() != binding_key.target() {
        return Err(
            PublicExportBindingClosureValidationError::HopTargetMismatch {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
                expected: binding_key.target(),
                actual: Box::new(hop_key.target()),
            },
        );
    }
    if hop_key.namespace() != binding_key.namespace() {
        return Err(
            PublicExportBindingClosureValidationError::HopNamespaceMismatch {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
                expected: binding_key.namespace(),
                actual: hop_key.namespace(),
            },
        );
    }
    if hop_key.role() != binding_key.role() {
        return Err(PublicExportBindingClosureValidationError::HopRoleMismatch {
            binding,
            route: route_index,
            hop: hop_index,
            hop_binding: hop.binding(),
            expected: binding_key.role(),
            actual: hop_key.role(),
        });
    }

    let surface = authority.public_bindings(hop.exporter()).ok_or(
        PublicExportBindingClosureValidationError::MissingProviderSurface {
            binding,
            route: route_index,
            hop: hop_index,
            provider: hop.exporter(),
        },
    )?;
    let record = surface.get(hop.binding()).ok_or(
        PublicExportBindingClosureValidationError::MissingProviderBinding {
            binding,
            route: route_index,
            hop: hop_index,
            provider: hop.exporter(),
            hop_binding: hop.binding(),
        },
    )?;

    let terminal = hop_index + 1 == route_hops.len();
    match (terminal, record.source()) {
        (true, ExportBindingSourceV1::DeclaredCurrent { declaration }) => {
            require_declared_target(hop.exporter(), hop.binding(), hop_key, *declaration)
        }
        (true, ExportBindingSourceV1::Reexport { .. }) => Err(
            PublicExportBindingClosureValidationError::TerminalIsReexport {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
            },
        ),
        (false, ExportBindingSourceV1::DeclaredCurrent { .. }) => Err(
            PublicExportBindingClosureValidationError::IntermediateIsDeclared {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
            },
        ),
        (false, ExportBindingSourceV1::Reexport { routes }) => {
            let suffix = &route_hops[hop_index + 1..];
            if contains_exact_suffix(routes, suffix) {
                Ok(())
            } else {
                Err(
                    PublicExportBindingClosureValidationError::MissingRouteSuffix {
                        binding,
                        route: route_index,
                        hop: hop_index,
                        hop_binding: hop.binding(),
                    },
                )
            }
        }
    }
}

fn require_declared_target(
    exporter: ConeIdentity,
    binding: PersistentExportBindingId,
    key: &ExportBindingKey,
    declaration: BindableEntity,
) -> Result<(), PublicExportBindingClosureValidationError> {
    if declaration == key.target() {
        Ok(())
    } else {
        Err(
            PublicExportBindingClosureValidationError::DeclaredTargetMismatch {
                exporter,
                binding,
                expected: key.target(),
                actual: Box::new(declaration),
            },
        )
    }
}

fn contains_exact_suffix(
    routes: &CanonicalReexportRoutesV1,
    suffix: &[ReexportRouteHopV1],
) -> bool {
    let first = suffix
        .first()
        .expect("an intermediate re-export always has a non-empty suffix");
    routes
        .routes()
        .iter()
        .any(|route| route.immediate_provider() == first.exporter() && route.hops() == suffix)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicExportBindingClosureValidationError {
    MissingCurrentBindingKey {
        binding: PersistentExportBindingId,
    },
    CurrentBindingExporterMismatch {
        binding: PersistentExportBindingId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    DeclaredTargetMismatch {
        exporter: ConeIdentity,
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
    ImmediateProviderNotDirect {
        binding: PersistentExportBindingId,
        route: usize,
        provider: ConeIdentity,
    },
    RouteExceedsClosure {
        binding: PersistentExportBindingId,
        route: usize,
        hops: usize,
        closure_nodes: usize,
    },
    MissingHopBindingKey {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
    HopBindingExporterMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: ConeIdentity,
        actual: Box<ConeIdentity>,
    },
    HopTargetMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
    HopNamespaceMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: BindingNamespace,
        actual: BindingNamespace,
    },
    HopRoleMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: BindingRole,
        actual: BindingRole,
    },
    MissingProviderSurface {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        provider: ConeIdentity,
    },
    MissingProviderBinding {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        provider: ConeIdentity,
        hop_binding: PersistentExportBindingId,
    },
    TerminalIsReexport {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
    IntermediateIsDeclared {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
    MissingRouteSuffix {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
}

impl fmt::Display for PublicExportBindingClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCurrentBindingKey { binding } => {
                write!(formatter, "public binding {binding} has no canonical key")
            }
            Self::CurrentBindingExporterMismatch {
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "public binding {binding} is exported by Cone {actual}, not current Cone {expected}"
            ),
            Self::DeclaredTargetMismatch {
                exporter,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "declared binding {binding} from Cone {exporter} targets {expected:?}, but its source names {actual:?}"
            ),
            Self::ImmediateProviderNotDirect {
                binding,
                route,
                provider,
            } => write!(
                formatter,
                "route {route} of binding {binding} begins at non-direct provider Cone {provider}"
            ),
            Self::RouteExceedsClosure {
                binding,
                route,
                hops,
                closure_nodes,
            } => write!(
                formatter,
                "route {route} of binding {binding} has {hops} hops but the closure has only {closure_nodes} nodes"
            ),
            Self::MissingHopBindingKey {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} names binding {hop_binding} without a canonical key"
            ),
            Self::HopBindingExporterMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} assigns binding {hop_binding} to Cone {expected}, but its key is exported by Cone {actual}"
            ),
            Self::HopTargetMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} uses binding {hop_binding} with target {actual:?}, expected {expected:?}"
            ),
            Self::HopNamespaceMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} uses binding {hop_binding} in namespace {actual:?}, expected {expected:?}"
            ),
            Self::HopRoleMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} uses binding {hop_binding} with role {actual:?}, expected {expected:?}"
            ),
            Self::MissingProviderSurface {
                binding,
                route,
                hop,
                provider,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} has no public surface for Cone {provider}"
            ),
            Self::MissingProviderBinding {
                binding,
                route,
                hop,
                provider,
                hop_binding,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} names absent binding {hop_binding} in Cone {provider}"
            ),
            Self::TerminalIsReexport {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "terminal hop {hop} of route {route} for binding {binding} is re-export binding {hop_binding}"
            ),
            Self::IntermediateIsDeclared {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "intermediate hop {hop} of route {route} for binding {binding} is declared binding {hop_binding}"
            ),
            Self::MissingRouteSuffix {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "intermediate binding {hop_binding} at hop {hop} of route {route} for binding {binding} does not publish the exact remaining suffix"
            ),
        }
    }
}

impl std::error::Error for PublicExportBindingClosureValidationError {}

#[cfg(test)]
mod tests;
