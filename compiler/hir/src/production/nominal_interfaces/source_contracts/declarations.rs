use super::*;
use std::collections::BTreeMap;

pub(in crate::production::nominal_interfaces) fn project_roots(
    export: &ExportHir,
    roots: &[SourceNominalId],
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<SourceNominalId, NominalInterfaceRecordV1>, Error> {
    let required = CanonicalSourceNominalIdsV1::from_complete_roots(export, roots, meter)?;
    project_required(export, &required, meter)
}

pub(in crate::production::nominal_interfaces) fn project_required(
    export: &ExportHir,
    required: &CanonicalSourceNominalIdsV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<SourceNominalId, NominalInterfaceRecordV1>, Error> {
    let mut records = BTreeMap::new();
    let path = WirePath::root();
    for local in locals(export) {
        work(meter, required.values().len())?;
        let identity = local
            .identity(export)
            .ok_or_else(|| invalid("sealed nominal has no typed identity"))?;
        let Some(source) = identity.source() else {
            continue;
        };
        let owner = source_nominal_id(source);
        if source.declaration().origin() != export.cone
            || required.values().binary_search(&owner).is_err()
        {
            continue;
        }
        let contract = projection::project(export, local, source, meter)?;
        let visibility = match local {
            LocalNominalId::Class(id) => export.classes[id].access.declared,
            LocalNominalId::Interface(id) => export.interfaces[id].access.declared,
            LocalNominalId::Struct(id) => export.structs[id].access.declared,
            LocalNominalId::Enum(id) => export.enums[id].access.declared,
            LocalNominalId::Object(id) => export.objects[id].access.declared,
        };
        // Source contracts are consumed here; the wire owns only the resulting
        // shared nominal record, not a parallel source-contract transcript.
        let order = dispatch_order::project(export, local, meter)?;
        let selections =
            crate::production::nominal_dispatch::project(export, local.owner(), meter)?;
        let record = NominalInterfaceRecordV1::from_source_contract(
            contract,
            visibility.into(),
            order,
            selections,
        )
        .map_err(invalid)?;
        meter.charge_collection_slots(1, &path).map_err(resource)?;
        work(meter, records.len())?;
        if records.insert(owner, record).is_some() {
            return Err(invalid("duplicate shared nominal declaration"));
        }
    }
    if records.len() != required.values().len() {
        return Err(invalid("required shared nominal declaration is absent"));
    }
    Ok(records)
}
