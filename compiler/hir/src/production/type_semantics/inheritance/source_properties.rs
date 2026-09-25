use super::source_errors::{invalid, resource};
use super::*;
use crate::production::{
    property_interfaces::source_property_representation, signatures::HirInterfaceSignatureProjector,
};
use scoop_identity::{DefinitionOriginSubject, PersistentPropertyId, SourceDeclarationKey};
use scoop_wire::WirePath;

mod contract;
mod nominals;
pub(in crate::production::type_semantics) use nominals::project as project_nominal;
mod required;
mod slots;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
) -> Result<CanonicalInheritanceSourcePropertiesV1, Error> {
    let mut required = required::project(export, inventory, selections)?;
    let mut records = Vec::new();
    let signatures = HirInterfaceSignatureProjector::new(export);
    for (id, _) in export.properties.iter() {
        let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        if !required.remove(&identity.id()) {
            continue;
        }

        scoop_wire::allocation::try_reserve(&mut records, 1, &WirePath::root())
            .map_err(resource)?;
        records.push(contract::project(export, id, &signatures)?);
    }
    if !required.is_empty() {
        return Err(invalid(
            "required inheritance property has no sealed source declaration",
        ));
    }
    CanonicalInheritanceSourcePropertiesV1::try_new(records).map_err(Error::SourceInventory)
}

fn declaration_access(
    export: &ExportHir,
    key: &SourceDeclarationKey,
    subject: DefinitionOriginSubject,
    visibility: DeclaredVisibility,
) -> Result<DeclarationAccessSourceV1, Error> {
    export
        .export_definition_origins
        .get(subject)
        .ok_or(Error::MissingDefinitionOrigin(subject))?;

    super::super::nominals::declaration_access_for_subject(export, key, subject, visibility.into())
}
