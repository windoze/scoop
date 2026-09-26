use super::*;
use std::collections::BTreeSet;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    required: &CanonicalPersistentIdsV1<PersistentPropertyId>,
) -> Result<Vec<NominalSupportPropertyInterfaceV1>, Error> {
    let path = WirePath::root();

    let mut required = required.values().iter().copied().collect::<BTreeSet<_>>();
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, required.len(), &path).map_err(resource)?;
    let signatures = HirInterfaceSignatureProjector::new(export);
    for (id, _) in export.properties.iter() {
        let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        if required.remove(&identity.id()) {
            records.push(contract::project(export, id, &signatures)?);
        }
    }
    if !required.is_empty() {
        return Err(invalid(
            "required nominal source property has no sealed declaration",
        ));
    }
    Ok(records)
}
