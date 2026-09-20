use super::*;
use std::collections::BTreeSet;

impl CanonicalNominalSourcePropertiesV1 {
    /// Projects an independently required declaration inventory from sealed
    /// HIR, including non-public properties and evaluated object constants.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        required: &CanonicalPersistentIdsV1<PersistentPropertyId>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Self::try_new(project(output.module(), required, meter)?, meter)
            .map_err(Error::SourceInventory)
    }
}
pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    required: &CanonicalPersistentIdsV1<PersistentPropertyId>,
    meter: &mut BudgetMeter,
) -> Result<Vec<NominalSupportPropertyInterfaceV1>, Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path).map_err(resource)?;
    meter
        .check_table_entries(required.values().len() as u64, &path)
        .map_err(resource)?;
    meter
        .charge_work(required.values().len() as u64, &path)
        .map_err(resource)?;
    meter
        .charge_collection_slots(required.values().len() as u64, &path)
        .map_err(resource)?;
    let mut required = required.values().iter().copied().collect::<BTreeSet<_>>();
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(&mut records, required.len(), &path)
        .map_err(resource)?;
    let signatures = HirInterfaceSignatureProjector::new(export);
    for (id, _) in export.properties.iter() {
        work(meter, required.len())?;
        let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        if required.remove(&identity.id()) {
            records.push(contract::project(export, id, &signatures, meter)?);
        }
    }
    if !required.is_empty() {
        return Err(invalid(
            "required nominal source property has no sealed declaration",
        ));
    }
    Ok(records)
}
