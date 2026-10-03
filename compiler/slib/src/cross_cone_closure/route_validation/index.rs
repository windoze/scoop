use std::sync::Arc;

use scoop_hir::{CrossConeHirInterfaceSectionV1, ExportBindingSourceV1, ReexportRouteV1};
use scoop_identity::{ExportBindingKey, PersistentExportBindingId, ValidatedIdentityGraph};
use scoop_wire::{WireError, WireErrorKind, WirePath};

use super::reserve_route_slots;

pub(super) fn binding_keys(
    identities: &ValidatedIdentityGraph,
    interface: &CrossConeHirInterfaceSectionV1,

    path: &WirePath,
) -> Result<Vec<(PersistentExportBindingId, Arc<ExportBindingKey>)>, WireError> {
    let mut count = 0_usize;
    visit_ids(interface, path, |_, path| {
        count = count
            .checked_add(1)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        Ok(())
    })?;
    let mut ids = Vec::new();
    reserve_route_slots(&mut ids, count, path)?;
    visit_ids(interface, path, |id, _| {
        ids.push(id);
        Ok(())
    })?;

    ids.sort_unstable();
    ids.dedup();

    let mut keys = Vec::new();
    reserve_route_slots(&mut keys, ids.len(), path)?;
    for binding in ids {
        // The identity graph performs two typed hash lookups and shares an Arc.

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

    path: &WirePath,
    mut visit: impl FnMut(PersistentExportBindingId, &WirePath) -> Result<(), WireError>,
) -> Result<(), WireError> {
    let bindings = interface.public_bindings().records();
    let binding_path = path.clone().field(9);

    for (index, record) in bindings.iter().enumerate() {
        let path = binding_path.clone().index(index as u64);

        visit(record.binding(), &path)?;
        if let ExportBindingSourceV1::Reexport { routes } = record.source() {
            let path = path.field(2).field(1);

            for (index, route) in routes.routes().iter().enumerate() {
                visit_route(route, &path.clone().index(index as u64), &mut visit)?;
            }
        }
    }
    let references = interface.external_references().records();
    let reference_path = path.clone().field(10);

    for (index, reference) in references.iter().enumerate() {
        let path = reference_path.clone().index(index as u64);

        let path = path.field(4);

        for (index, witness) in reference.witnesses().witnesses().iter().enumerate() {
            visit_route(
                witness.route(),
                &path.clone().index(index as u64),
                &mut visit,
            )?;
        }
    }
    Ok(())
}

fn visit_route(
    route: &ReexportRouteV1,

    path: &WirePath,
    visit: &mut impl FnMut(PersistentExportBindingId, &WirePath) -> Result<(), WireError>,
) -> Result<(), WireError> {
    let path = path.clone().field(2);

    for (index, hop) in route.hops().iter().enumerate() {
        let path = path.clone().index(index as u64);

        visit(hop.binding(), &path)?;
    }
    Ok(())
}
