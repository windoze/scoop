use std::sync::Arc;

use scoop_hir::{CrossConeHirInterfaceSectionV1, ExportBindingSourceV1, ReexportRouteV1};
use scoop_identity::{ExportBindingKey, PersistentExportBindingId, ValidatedIdentityGraph};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::reserve_route_slots;

pub(super) fn binding_keys(
    identities: &ValidatedIdentityGraph,
    interface: &CrossConeHirInterfaceSectionV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Vec<(PersistentExportBindingId, Arc<ExportBindingKey>)>, WireError> {
    let mut count = 0_usize;
    visit_ids(interface, meter, path, |_, meter, path| {
        count = count
            .checked_add(1)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        meter.check_table_entries(count as u64, path)
    })?;
    let mut ids = Vec::new();
    reserve_route_slots(&mut ids, count, meter, path)?;
    visit_ids(interface, meter, path, |id, _, _| {
        ids.push(id);
        Ok(())
    })?;
    let count = ids.len() as u64;
    meter.charge_work(
        count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
        path,
    )?;
    ids.sort_unstable();
    ids.dedup();

    let mut keys = Vec::new();
    reserve_route_slots(&mut keys, ids.len(), meter, path)?;
    for binding in ids {
        // The identity graph performs two typed hash lookups and shares an Arc.
        meter.charge_work(3, path)?;
        if let Ok(key) =
            identities.canonical_key::<PersistentExportBindingId, ExportBindingKey>(binding)
        {
            keys.push((binding, key));
        }
    }
    Ok(keys)
}

fn visit_ids(
    interface: &CrossConeHirInterfaceSectionV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
    mut visit: impl FnMut(
        PersistentExportBindingId,
        &mut BudgetMeter,
        &WirePath,
    ) -> Result<(), WireError>,
) -> Result<(), WireError> {
    let bindings = interface.public_bindings().records();
    let binding_path = path.clone().field(9);
    meter.check_table_entries(bindings.len() as u64, &binding_path)?;
    for (index, record) in bindings.iter().enumerate() {
        let path = binding_path.clone().index(index as u64);
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        visit(record.binding(), meter, &path)?;
        if let ExportBindingSourceV1::Reexport { routes } = record.source() {
            let path = path.field(2).field(1);
            meter.check_table_entries(routes.routes().len() as u64, &path)?;
            for (index, route) in routes.routes().iter().enumerate() {
                visit_route(route, meter, &path.clone().index(index as u64), &mut visit)?;
            }
        }
    }
    let references = interface.external_references().records();
    let reference_path = path.clone().field(10);
    meter.check_table_entries(references.len() as u64, &reference_path)?;
    for (index, reference) in references.iter().enumerate() {
        let path = reference_path.clone().index(index as u64);
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        let path = path.field(4);
        meter.check_table_entries(reference.witnesses().witnesses().len() as u64, &path)?;
        for (index, witness) in reference.witnesses().witnesses().iter().enumerate() {
            visit_route(
                witness.route(),
                meter,
                &path.clone().index(index as u64),
                &mut visit,
            )?;
        }
    }
    Ok(())
}

fn visit_route(
    route: &ReexportRouteV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
    visit: &mut impl FnMut(
        PersistentExportBindingId,
        &mut BudgetMeter,
        &WirePath,
    ) -> Result<(), WireError>,
) -> Result<(), WireError> {
    meter.charge_nodes(1, path)?;
    meter.charge_work(1, path)?;
    let path = path.clone().field(2);
    meter.check_table_entries(route.hops().len() as u64, &path)?;
    meter.check_semantic_depth(route.hops().len() as u64, &path)?;
    for (index, hop) in route.hops().iter().enumerate() {
        let path = path.clone().index(index as u64);
        meter.charge_edges(1, &path)?;
        meter.charge_work(1, &path)?;
        visit(hop.binding(), meter, &path)?;
    }
    Ok(())
}
