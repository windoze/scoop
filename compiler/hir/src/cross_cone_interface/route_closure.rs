mod errors;
mod hops;
pub use errors::PublicExportBindingClosureValidationError;

use scoop_identity::{ConeIdentity, ExportBindingKey, PersistentExportBindingId};

use super::{CanonicalPublicExportBindingsV1, ExportBindingSourceV1};
use crate::CanonicalReexportRoutesV1;

/// Read-only queries scoped to the current artifact's actual dependency graph.
pub trait PublicExportBindingClosureAuthority {
    fn closure_node_count(&self) -> usize;

    fn is_direct_dependency(&self, provider: ConeIdentity) -> bool;

    fn binding_key(&self, binding: PersistentExportBindingId) -> Option<&ExportBindingKey>;

    fn public_bindings(&self, exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1>;
}

impl CanonicalPublicExportBindingsV1 {
    /// Checks direct providers, typed binding relations, terminal declarations,
    /// and exact suffix continuity against the complete artifact closure.
    pub fn validate_route_closure<A: PublicExportBindingClosureAuthority>(
        &self,
        current: ConeIdentity,
        authority: &A,
    ) -> Result<(), PublicExportBindingClosureValidationError> {
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
                    hops::require_declared_target(current, binding, key, *declaration)?;
                }
                ExportBindingSourceV1::Reexport { routes } => {
                    validate_routes(binding, key, routes, authority)?;
                }
            }
        }
        Ok(())
    }
}

fn validate_routes<A: PublicExportBindingClosureAuthority>(
    binding: PersistentExportBindingId,
    binding_key: &ExportBindingKey,
    routes: &CanonicalReexportRoutesV1,
    authority: &A,
) -> Result<(), PublicExportBindingClosureValidationError> {
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
            hops::validate_hop(
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

#[cfg(test)]
mod tests;
