use std::fmt;

use scoop_identity::{
    BindableEntity, BindingNamespace, BindingRole, BindingTarget, ConeIdentity, ExportBindingKey,
    PersistentExportBindingId,
};

use super::DependencyBindingWitnessV1;
use crate::{
    CanonicalReexportRoutesV1, ExportBindingSourceV1, PublicExportBindingClosureAuthority,
    ReexportRouteHopV1,
};

impl DependencyBindingWitnessV1 {
    pub fn validate_semantics<A>(
        &self,
        root: BindingTarget,
        authority: &A,
    ) -> Result<(), DependencyBindingWitnessSemanticValidationError>
    where
        A: PublicExportBindingClosureAuthority,
    {
        let route = self.route();
        if !authority.is_direct_dependency(route.immediate_provider()) {
            return Err(
                DependencyBindingWitnessSemanticValidationError::ImmediateProviderNotDirect {
                    provider: route.immediate_provider(),
                },
            );
        }
        if route.hops().len() > authority.closure_node_count() {
            return Err(
                DependencyBindingWitnessSemanticValidationError::RouteExceedsClosure {
                    hops: route.hops().len(),
                    closure_nodes: authority.closure_node_count(),
                },
            );
        }

        for (hop_index, hop) in route.hops().iter().copied().enumerate() {
            validate_hop(root, hop_index, route.hops(), hop, authority)?;
        }
        Ok(())
    }
}

fn validate_hop<A>(
    root: BindingTarget,
    hop_index: usize,
    route_hops: &[ReexportRouteHopV1],
    hop: ReexportRouteHopV1,
    authority: &A,
) -> Result<(), DependencyBindingWitnessSemanticValidationError>
where
    A: PublicExportBindingClosureAuthority,
{
    let binding = hop.binding();
    let key = authority.binding_key(binding).ok_or(
        DependencyBindingWitnessSemanticValidationError::MissingHopBindingKey {
            hop: hop_index,
            binding,
        },
    )?;
    validate_binding_key(root, hop_index, hop, key)?;

    let surface = authority.public_bindings(hop.exporter()).ok_or(
        DependencyBindingWitnessSemanticValidationError::MissingProviderSurface {
            hop: hop_index,
            provider: hop.exporter(),
        },
    )?;
    let record = surface.get(binding).ok_or(
        DependencyBindingWitnessSemanticValidationError::MissingProviderBinding {
            hop: hop_index,
            provider: hop.exporter(),
            binding,
        },
    )?;

    let terminal = hop_index + 1 == route_hops.len();
    match (terminal, record.source()) {
        (true, ExportBindingSourceV1::DeclaredCurrent { declaration }) => {
            if *declaration == root.target() {
                Ok(())
            } else {
                Err(
                    DependencyBindingWitnessSemanticValidationError::DeclaredTargetMismatch {
                        hop: hop_index,
                        binding,
                        expected: root.target(),
                        actual: Box::new(*declaration),
                    },
                )
            }
        }
        (true, ExportBindingSourceV1::Reexport { .. }) => Err(
            DependencyBindingWitnessSemanticValidationError::TerminalIsReexport {
                hop: hop_index,
                binding,
            },
        ),
        (false, ExportBindingSourceV1::DeclaredCurrent { .. }) => Err(
            DependencyBindingWitnessSemanticValidationError::IntermediateIsDeclared {
                hop: hop_index,
                binding,
            },
        ),
        (false, ExportBindingSourceV1::Reexport { routes }) => {
            let suffix = &route_hops[hop_index + 1..];
            if contains_exact_suffix(routes, suffix) {
                Ok(())
            } else {
                Err(
                    DependencyBindingWitnessSemanticValidationError::MissingRouteSuffix {
                        hop: hop_index,
                        binding,
                    },
                )
            }
        }
    }
}

fn validate_binding_key(
    root: BindingTarget,
    hop_index: usize,
    hop: ReexportRouteHopV1,
    key: &ExportBindingKey,
) -> Result<(), DependencyBindingWitnessSemanticValidationError> {
    let binding = hop.binding();
    if key.exporter() != hop.exporter() {
        return Err(
            DependencyBindingWitnessSemanticValidationError::HopBindingExporterMismatch {
                hop: hop_index,
                binding,
                expected: hop.exporter(),
                actual: key.exporter(),
            },
        );
    }
    if key.target() != root.target() {
        return Err(
            DependencyBindingWitnessSemanticValidationError::HopTargetMismatch {
                hop: hop_index,
                binding,
                expected: root.target(),
                actual: Box::new(key.target()),
            },
        );
    }
    if key.namespace() != root.namespace() {
        return Err(
            DependencyBindingWitnessSemanticValidationError::HopNamespaceMismatch {
                hop: hop_index,
                binding,
                expected: root.namespace(),
                actual: key.namespace(),
            },
        );
    }
    if key.role() != root.role() {
        return Err(
            DependencyBindingWitnessSemanticValidationError::HopRoleMismatch {
                hop: hop_index,
                binding,
                expected: root.role(),
                actual: key.role(),
            },
        );
    }
    Ok(())
}

fn contains_exact_suffix(
    routes: &CanonicalReexportRoutesV1,
    suffix: &[ReexportRouteHopV1],
) -> bool {
    let first = suffix
        .first()
        .expect("an intermediate dependency witness always has a non-empty suffix");
    routes
        .routes()
        .iter()
        .any(|route| route.immediate_provider() == first.exporter() && route.hops() == suffix)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyBindingWitnessSemanticValidationError {
    ImmediateProviderNotDirect {
        provider: ConeIdentity,
    },
    RouteExceedsClosure {
        hops: usize,
        closure_nodes: usize,
    },
    MissingHopBindingKey {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    HopBindingExporterMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    HopTargetMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
    HopNamespaceMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindingNamespace,
        actual: BindingNamespace,
    },
    HopRoleMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindingRole,
        actual: BindingRole,
    },
    MissingProviderSurface {
        hop: usize,
        provider: ConeIdentity,
    },
    MissingProviderBinding {
        hop: usize,
        provider: ConeIdentity,
        binding: PersistentExportBindingId,
    },
    TerminalIsReexport {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    IntermediateIsDeclared {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    MissingRouteSuffix {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    DeclaredTargetMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
}

impl fmt::Display for DependencyBindingWitnessSemanticValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImmediateProviderNotDirect { provider } => write!(
                formatter,
                "dependency binding witness begins at non-direct provider Cone {provider}"
            ),
            Self::RouteExceedsClosure {
                hops,
                closure_nodes,
            } => write!(
                formatter,
                "dependency binding witness has {hops} hops but the closure has only {closure_nodes} nodes"
            ),
            Self::MissingHopBindingKey { hop, binding } => write!(
                formatter,
                "dependency binding witness hop {hop} names binding {binding} without a canonical key"
            ),
            Self::HopBindingExporterMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} assigns binding {binding} to Cone {expected}, but its key is exported by Cone {actual}"
            ),
            Self::HopTargetMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} uses binding {binding} with target {actual:?}, expected {expected:?}"
            ),
            Self::HopNamespaceMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} uses binding {binding} in namespace {actual:?}, expected {expected:?}"
            ),
            Self::HopRoleMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} uses binding {binding} with role {actual:?}, expected {expected:?}"
            ),
            Self::MissingProviderSurface { hop, provider } => write!(
                formatter,
                "dependency binding witness hop {hop} has no public surface for Cone {provider}"
            ),
            Self::MissingProviderBinding {
                hop,
                provider,
                binding,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} names absent binding {binding} in Cone {provider}"
            ),
            Self::TerminalIsReexport { hop, binding } => write!(
                formatter,
                "dependency binding witness terminal hop {hop} is re-export binding {binding}"
            ),
            Self::IntermediateIsDeclared { hop, binding } => write!(
                formatter,
                "dependency binding witness intermediate hop {hop} is declared binding {binding}"
            ),
            Self::MissingRouteSuffix { hop, binding } => write!(
                formatter,
                "dependency binding witness intermediate binding {binding} at hop {hop} does not publish the exact remaining suffix"
            ),
            Self::DeclaredTargetMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness terminal hop {hop} binding {binding} declares {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for DependencyBindingWitnessSemanticValidationError {}

#[cfg(test)]
mod tests;
