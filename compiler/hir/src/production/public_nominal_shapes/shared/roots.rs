use super::*;
use scoop_identity::BindableEntity;

pub(super) fn current(
    export: &ExportHir,
    meter: &mut BudgetMeter,
) -> Result<Vec<SourceNominalId>, PublicNominalShapeProjectionError> {
    let path = WirePath::root();
    let mut keys = BTreeMap::new();
    for record in export.export_binding_identities.iter() {
        meter
            .check_table_entries(keys.len() as u64 + 1, &path)
            .map_err(resource)?;
        meter
            .charge_work(1 + u64::from(keys.len().max(1).ilog2()), &path)
            .map_err(resource)?;
        meter.charge_collection_slots(1, &path).map_err(resource)?;
        keys.insert(record.id(), record.key());
    }
    let mut roots = BTreeSet::new();
    for record in export.public_export_bindings.records() {
        meter
            .charge_work(1 + u64::from(keys.len().max(1).ilog2()), &path)
            .map_err(resource)?;
        let key = keys.get(&record.binding()).ok_or(
            PublicNominalShapeProjectionError::MissingBinding(record.binding()),
        )?;
        if key.exporter() != export.cone {
            continue;
        }
        let ExportBindingSourceV1::DeclaredCurrent { declaration } = record.source() else {
            continue;
        };
        let source = match declaration {
            BindableEntity::Type(id) => SourceNominalId::Concrete(*id),
            BindableEntity::GenericType(id) => SourceNominalId::GenericTemplate(*id),
            _ => continue,
        };
        meter
            .charge_work(1 + u64::from(roots.len().max(1).ilog2()), &path)
            .map_err(resource)?;
        if !roots.contains(&source) {
            meter
                .check_table_entries(roots.len() as u64 + 1, &path)
                .map_err(resource)?;
            meter.charge_collection_slots(1, &path).map_err(resource)?;
            roots.insert(source);
        }
    }
    let mut values = Vec::new();
    meter
        .try_reserve_collection_slots(&mut values, roots.len(), &path)
        .map_err(resource)?;
    values.extend(roots);
    Ok(values)
}
