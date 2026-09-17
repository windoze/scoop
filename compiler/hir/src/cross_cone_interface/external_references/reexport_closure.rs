use std::fmt;

use scoop_identity::{ConeIdentity, PersistentExportBindingId};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{
    CanonicalExternalHirReferencesV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};
use crate::{CanonicalPublicExportBindingsV1, ExportBindingSourceV1, ReexportRouteV1};

impl CanonicalExternalHirReferencesV1 {
    /// Validates the exact external target and reconstructible witness closure
    /// contributed by re-export records in the current public surface.
    ///
    /// Other source-name roles may contribute additional valid witnesses for
    /// the same target. In their absence, the witness set must be exactly the
    /// union of the re-export routes found here.
    pub fn validate_reexport_closure<A, E>(
        &self,
        bindings: &CanonicalPublicExportBindingsV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExternalHirReexportClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut seen_records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut seen_records, self.records().len(), path)
            .map_err(ExternalHirReexportClosureValidationError::Resource)?;
        seen_records.resize(self.records().len(), false);

        let mut seen_witnesses = Vec::new();
        meter
            .try_reserve_collection_slots(&mut seen_witnesses, self.records().len(), path)
            .map_err(ExternalHirReexportClosureValidationError::Resource)?;
        for record in self.records() {
            let mut seen = Vec::new();
            meter
                .try_reserve_collection_slots(&mut seen, record.witnesses().witnesses().len(), path)
                .map_err(ExternalHirReexportClosureValidationError::Resource)?;
            seen.resize(record.witnesses().witnesses().len(), false);
            seen_witnesses.push(seen);
        }

        for (binding_index, binding) in bindings.records().iter().enumerate() {
            let ExportBindingSourceV1::Reexport { routes } = binding.source() else {
                continue;
            };
            charge_node(meter, path)?;
            let binding_id = binding.binding();
            let target = authority
                .binding_key(binding_id)
                .map(|key| ExternalHirTargetV1::from(key.target()))
                .ok_or(
                    ExternalHirReexportClosureValidationError::MissingBindingKey {
                        binding_index,
                        binding: binding_id,
                    },
                )?;
            let origin = authority
                .external_hir_target_origin(target)
                .map_err(
                    |error| ExternalHirReexportClosureValidationError::TargetOrigin {
                        binding_index,
                        target,
                        error,
                    },
                )?;
            if origin == authority.current_cone() {
                return Err(
                    ExternalHirReexportClosureValidationError::CurrentConeTarget {
                        binding_index,
                        target,
                        current: origin,
                    },
                );
            }

            let record_index = find_record(self, target, meter, path)?.ok_or(
                ExternalHirReexportClosureValidationError::MissingReference {
                    binding_index,
                    target,
                },
            )?;
            let record = &self.records()[record_index];
            if record.origin() != origin {
                return Err(ExternalHirReexportClosureValidationError::OriginMismatch {
                    binding_index,
                    record_index,
                    target,
                    expected: origin,
                    actual: record.origin(),
                });
            }
            if !record
                .roles()
                .contains(ExternalHirReferenceRoleV1::ReexportTarget)
            {
                return Err(ExternalHirReexportClosureValidationError::MissingRole {
                    binding_index,
                    record_index,
                    target,
                });
            }
            seen_records[record_index] = true;

            for (route_index, route) in routes.routes().iter().enumerate() {
                charge_edge(meter, path)?;
                let witness_index = find_witness(record, route, meter, path)?.ok_or(
                    ExternalHirReexportClosureValidationError::MissingWitness {
                        binding_index,
                        route_index,
                        record_index,
                        target,
                    },
                )?;
                seen_witnesses[record_index][witness_index] = true;
            }
        }

        for (record_index, record) in self.records().iter().enumerate() {
            meter
                .charge_work(1, path)
                .map_err(ExternalHirReexportClosureValidationError::Resource)?;
            if record
                .roles()
                .contains(ExternalHirReferenceRoleV1::ReexportTarget)
                && !seen_records[record_index]
            {
                return Err(ExternalHirReexportClosureValidationError::ExtraRole {
                    record_index,
                    target: record.target(),
                });
            }

            let has_non_reexport_source_role = [
                ExternalHirReferenceRoleV1::AliasTarget,
                ExternalHirReferenceRoleV1::DefaultDependency,
                ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            ]
            .into_iter()
            .any(|role| record.roles().contains(role));
            if has_non_reexport_source_role {
                continue;
            }
            for (witness_index, seen) in seen_witnesses[record_index].iter().copied().enumerate() {
                meter
                    .charge_work(1, path)
                    .map_err(ExternalHirReexportClosureValidationError::Resource)?;
                if !seen {
                    return Err(ExternalHirReexportClosureValidationError::ExtraWitness {
                        record_index,
                        witness_index,
                        target: record.target(),
                    });
                }
            }
        }

        Ok(())
    }
}

fn find_record<E>(
    references: &CanonicalExternalHirReferencesV1,
    target: ExternalHirTargetV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Option<usize>, ExternalHirReexportClosureValidationError<E>> {
    let mut start = 0;
    let mut end = references.records().len();
    while start < end {
        meter
            .charge_work(1, path)
            .map_err(ExternalHirReexportClosureValidationError::Resource)?;
        let middle = start + (end - start) / 2;
        match references.records()[middle].target().cmp(&target) {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}

fn find_witness<E>(
    record: &super::ExternalHirReferenceV1,
    route: &ReexportRouteV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Option<usize>, ExternalHirReexportClosureValidationError<E>> {
    let witnesses = record.witnesses().witnesses();
    let mut start = 0;
    let mut end = witnesses.len();
    while start < end {
        meter
            .charge_work(1, path)
            .map_err(ExternalHirReexportClosureValidationError::Resource)?;
        let middle = start + (end - start) / 2;
        match witnesses[middle].route().cmp(route) {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}

fn charge_node<E>(
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExternalHirReexportClosureValidationError<E>> {
    meter
        .check_semantic_depth(1, path)
        .map_err(ExternalHirReexportClosureValidationError::Resource)?;
    meter
        .charge_nodes(1, path)
        .map_err(ExternalHirReexportClosureValidationError::Resource)?;
    meter
        .charge_work(1, path)
        .map_err(ExternalHirReexportClosureValidationError::Resource)
}

fn charge_edge<E>(
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExternalHirReexportClosureValidationError<E>> {
    meter
        .check_semantic_depth(2, path)
        .map_err(ExternalHirReexportClosureValidationError::Resource)?;
    meter
        .charge_edges(1, path)
        .map_err(ExternalHirReexportClosureValidationError::Resource)?;
    meter
        .charge_work(1, path)
        .map_err(ExternalHirReexportClosureValidationError::Resource)
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirReexportClosureValidationError<E> {
    MissingBindingKey {
        binding_index: usize,
        binding: PersistentExportBindingId,
    },
    TargetOrigin {
        binding_index: usize,
        target: ExternalHirTargetV1,
        error: E,
    },
    CurrentConeTarget {
        binding_index: usize,
        target: ExternalHirTargetV1,
        current: ConeIdentity,
    },
    MissingReference {
        binding_index: usize,
        target: ExternalHirTargetV1,
    },
    OriginMismatch {
        binding_index: usize,
        record_index: usize,
        target: ExternalHirTargetV1,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    MissingRole {
        binding_index: usize,
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    MissingWitness {
        binding_index: usize,
        route_index: usize,
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    ExtraRole {
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    ExtraWitness {
        record_index: usize,
        witness_index: usize,
        target: ExternalHirTargetV1,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for ExternalHirReexportClosureValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBindingKey {
                binding_index,
                binding,
            } => write!(
                formatter,
                "re-export binding {binding} at index {binding_index} has no canonical key"
            ),
            Self::TargetOrigin {
                binding_index,
                target,
                error,
            } => write!(
                formatter,
                "re-export target {target:?} at binding index {binding_index} has no canonical origin: {error}"
            ),
            Self::CurrentConeTarget {
                binding_index,
                target,
                current,
            } => write!(
                formatter,
                "re-export target {target:?} at binding index {binding_index} belongs to current Cone {current}"
            ),
            Self::MissingReference {
                binding_index,
                target,
            } => write!(
                formatter,
                "re-export target {target:?} at binding index {binding_index} is absent from the external HIR reference table"
            ),
            Self::OriginMismatch {
                binding_index,
                record_index,
                target,
                expected,
                actual,
            } => write!(
                formatter,
                "re-export target {target:?} at binding index {binding_index} uses external record {record_index} with origin {actual}, expected {expected}"
            ),
            Self::MissingRole {
                binding_index,
                record_index,
                target,
            } => write!(
                formatter,
                "re-export target {target:?} at binding index {binding_index} uses external record {record_index} without ReexportTarget role"
            ),
            Self::MissingWitness {
                binding_index,
                route_index,
                record_index,
                target,
            } => write!(
                formatter,
                "re-export route {route_index} for target {target:?} at binding index {binding_index} is absent from external record {record_index} witnesses"
            ),
            Self::ExtraRole {
                record_index,
                target,
            } => write!(
                formatter,
                "external record {record_index} for target {target:?} has ReexportTarget role without a re-export binding"
            ),
            Self::ExtraWitness {
                record_index,
                witness_index,
                target,
            } => write!(
                formatter,
                "external record {record_index} for re-export target {target:?} has unused witness {witness_index}"
            ),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "external re-export closure resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirReexportClosureValidationError<E>
{
}

#[cfg(test)]
mod tests;
