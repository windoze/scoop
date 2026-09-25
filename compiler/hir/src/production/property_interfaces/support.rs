use super::*;
use crate::{PropertyDeclarationId, PropertyDeclarationRecordV1};
use std::collections::BTreeSet;

pub(super) fn project(
    export: &ExportHir,
    projector: &HirInterfaceSignatureProjector<'_>,
    mut required: BTreeSet<PropertyDeclarationId>,
) -> Result<Vec<PropertyDeclarationRecordV1>, PropertyInterfaceBuildError> {
    use PropertyInterfaceBuildError as Error;
    let path = WirePath::root().field(4);
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, required.len(), &path)
        .map_err(Error::Resource)?;
    for (id, _) in export.properties.iter() {
        let Some(identity) = export.property_identities.get(id) else {
            continue;
        };
        if required.remove(&persistent_property_owner(identity)) {
            records.push(declaration::project(export, projector, id)?);
        }
    }
    if let Some(id) = required.first() {
        return Err(Error::MissingSupport(*id));
    }
    Ok(records)
}

impl CanonicalPropertyInterfacesV1 {
    pub(in crate::production) fn from_nominal_declarations(
        export: &ExportHir,
        nominals: &CanonicalNominalInterfacesV1,
    ) -> Result<Self, PropertyInterfaceBuildError> {
        use PropertyInterfaceBuildError as Error;
        let required = nominals
            .declared_source_properties()
            .map_err(Error::Inventory)?;
        let records = project(
            export,
            &HirInterfaceSignatureProjector::new(export),
            required.into_keys().collect(),
        )?;
        let table = Self::with_support(Vec::new(), records).map_err(Error::Table)?;
        table
            .validate_declaration_inventory(nominals)
            .map_err(Error::Inventory)?;
        Ok(table)
    }
}
