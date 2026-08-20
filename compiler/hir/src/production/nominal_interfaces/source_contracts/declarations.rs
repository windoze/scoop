use super::*;
use std::collections::BTreeMap;

pub(in crate::production::nominal_interfaces) fn project_required(
    export: &ExportHir,
    required: &CanonicalSourceNominalIdsV1,
) -> Result<BTreeMap<SourceNominalId, NominalInterfaceRecordV1>, Error> {
    let mut records = BTreeMap::new();

    for local in locals(export) {
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
        let record = projection::project(export, local, source)?;

        if records.insert(owner, record).is_some() {
            return Err(invalid("duplicate shared nominal declaration"));
        }
    }
    if records.len() != required.values().len() {
        return Err(invalid("required shared nominal declaration is absent"));
    }
    Ok(records)
}
