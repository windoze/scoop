mod errors;
pub use errors::DependencyBindingWitnessSemanticValidationError;

use scoop_wire::{BudgetMeter, WirePath};

use scoop_identity::{BindingTarget, ExportBindingKey};

use super::DependencyBindingWitnessV1;
use crate::{ExportBindingSourceV1, PublicExportBindingClosureAuthority, ReexportRouteHopV1};

impl DependencyBindingWitnessV1 {
    pub fn validate_semantics<A>(
        &self,
        root: BindingTarget,
        authority: &A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DependencyBindingWitnessSemanticValidationError>
    where
        A: PublicExportBindingClosureAuthority,
    {
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        let route = self.route();
        if !authority.is_direct_dependency(route.immediate_provider(), meter, path)? {
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

        let path = path.clone().field(2);
        meter.check_table_entries(route.hops().len() as u64, &path)?;
        meter.check_semantic_depth(route.hops().len() as u64, &path)?;
        for (hop_index, hop) in route.hops().iter().copied().enumerate() {
            let path = path.clone().index(hop_index as u64);
            meter.charge_edges(1, &path)?;
            meter.charge_work(1, &path)?;
            validate_hop(root, hop_index, route.hops(), hop, authority, meter, &path)?;
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
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), DependencyBindingWitnessSemanticValidationError>
where
    A: PublicExportBindingClosureAuthority,
{
    let binding = hop.binding();
    let key = authority.binding_key(binding, meter, path)?.ok_or(
        DependencyBindingWitnessSemanticValidationError::MissingHopBindingKey {
            hop: hop_index,
            binding,
        },
    )?;
    validate_binding_key(root, hop_index, hop, key)?;

    let surface = authority
        .public_bindings(hop.exporter(), meter, path)?
        .ok_or(
            DependencyBindingWitnessSemanticValidationError::MissingProviderSurface {
                hop: hop_index,
                provider: hop.exporter(),
            },
        )?;
    let record = surface.get_metered(binding, meter, path)?.ok_or(
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
            if routes.contains_exact_suffix_metered(suffix, meter, path)? {
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

#[cfg(test)]
mod tests;
