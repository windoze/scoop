mod errors;
mod hops;
pub use errors::PublicExportBindingClosureValidationError;

use scoop_identity::{ConeIdentity, ExportBindingKey, PersistentExportBindingId};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{CanonicalPublicExportBindingsV1, ExportBindingSourceV1};
use crate::CanonicalReexportRoutesV1;

/// Read-only queries scoped to the current artifact's actual dependency graph.
/// Every lookup charges its work to the caller's existing artifact meter.
pub trait PublicExportBindingClosureAuthority {
    fn closure_node_count(&self) -> usize;

    fn is_direct_dependency(
        &self,
        provider: ConeIdentity,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, WireError>;

    fn binding_key(
        &self,
        binding: PersistentExportBindingId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<&ExportBindingKey>, WireError>;

    fn public_bindings(
        &self,
        exporter: ConeIdentity,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<&CanonicalPublicExportBindingsV1>, WireError>;
}

impl CanonicalPublicExportBindingsV1 {
    /// Checks direct providers, typed binding relations, terminal declarations,
    /// and exact suffix continuity against the complete artifact closure.
    pub fn validate_route_closure<A: PublicExportBindingClosureAuthority>(
        &self,
        current: ConeIdentity,
        authority: &A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), PublicExportBindingClosureValidationError> {
        meter.charge_work(1, path)?;
        meter.check_table_entries(self.records().len() as u64, path)?;
        for (index, record) in self.records().iter().enumerate() {
            let path = path.clone().index(index as u64);
            meter.charge_nodes(1, &path)?;
            meter.charge_work(1, &path)?;
            let binding = record.binding();
            let key = authority.binding_key(binding, meter, &path)?.ok_or(
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
                    validate_routes(
                        binding,
                        key,
                        routes,
                        authority,
                        meter,
                        &path.field(2).field(1),
                    )?;
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
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), PublicExportBindingClosureValidationError> {
    meter.check_table_entries(routes.routes().len() as u64, path)?;
    for (route_index, route) in routes.routes().iter().enumerate() {
        let path = path.clone().index(route_index as u64);
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        if !authority.is_direct_dependency(route.immediate_provider(), meter, &path)? {
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
        let path = path.field(2);
        meter.check_table_entries(route.hops().len() as u64, &path)?;
        meter.check_semantic_depth(route.hops().len() as u64, &path)?;
        for (hop_index, hop) in route.hops().iter().copied().enumerate() {
            let path = path.clone().index(hop_index as u64);
            meter.charge_edges(1, &path)?;
            meter.charge_work(1, &path)?;
            hops::validate_hop(
                binding,
                binding_key,
                route_index,
                hop_index,
                route.hops(),
                hop,
                authority,
                meter,
                &path,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
