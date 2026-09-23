use super::*;
use crate::{PropertyDeclarationId, PropertyDeclarationRecordV1, SourceNominalId};
use std::collections::BTreeMap;

pub(super) fn project(
    export: &ExportHir,
    projector: &HirInterfaceSignatureProjector<'_>,
    mut required: BTreeMap<PropertyDeclarationId, SourceNominalId>,
    meter: &mut BudgetMeter,
) -> Result<Vec<PropertyDeclarationRecordV1>, PropertyInterfaceBuildError> {
    use PropertyInterfaceBuildError as Error;
    let path = WirePath::root().field(4);
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(&mut records, required.len(), &path)
        .map_err(Error::Resource)?;
    for (id, _) in export.properties.iter() {
        query(meter, required.len())?;
        let Some(identity) = export.property_identities.get(id) else {
            continue;
        };
        if required
            .remove(&persistent_property_owner(identity))
            .is_some()
        {
            records.push(declaration::project(export, projector, id, meter)?);
        }
    }
    if let Some((id, _)) = required.first_key_value() {
        return Err(Error::MissingSupport(*id));
    }
    Ok(records)
}

impl CanonicalPropertyInterfacesV1 {
    pub(in crate::production) fn from_nominal_declarations(
        export: &ExportHir,
        nominals: &CanonicalNominalInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, PropertyInterfaceBuildError> {
        use PropertyInterfaceBuildError as Error;
        let required = nominals
            .declared_source_properties(meter)
            .map_err(Error::Inventory)?;
        let records = project(
            export,
            &HirInterfaceSignatureProjector::new(export),
            required,
            meter,
        )?;
        let table = Self::with_support(Vec::new(), records).map_err(Error::Table)?;
        table
            .validate_declaration_inventory(nominals, meter)
            .map_err(Error::Inventory)?;
        Ok(table)
    }
}
