use super::source_resources::{self as resources, invalid, resource, work};
use super::*;
use crate::production::{
    property_interfaces::source_property_representation, signatures::HirInterfaceSignatureProjector,
};
use scoop_identity::{DefinitionOriginSubject, PersistentPropertyId, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};

mod contract;
mod nominals;
mod required;
mod slots;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourcePropertiesV1, Error> {
    let mut required = required::project(export, inventory, selections, meter)?;
    let mut records = Vec::new();
    let signatures = HirInterfaceSignatureProjector::new(export);
    for (id, _) in export.properties.iter() {
        work(meter, required.len())?;
        let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        if !required.remove(&identity.id()) {
            continue;
        }
        meter
            .check_table_entries(records.len() as u64 + 1, &WirePath::root())
            .map_err(resource)?;
        meter
            .try_reserve_collection_slots(&mut records, 1, &WirePath::root())
            .map_err(resource)?;
        records.push(contract::project(export, id, &signatures, meter)?);
    }
    if !required.is_empty() {
        return Err(invalid(
            "required inheritance property has no sealed source declaration",
        ));
    }
    CanonicalInheritanceSourcePropertiesV1::try_new(records, meter).map_err(Error::SourceInventory)
}

fn declaration_access(
    export: &ExportHir,
    key: &SourceDeclarationKey,
    subject: DefinitionOriginSubject,
    visibility: DeclaredVisibility,
    meter: &mut BudgetMeter,
) -> Result<DeclarationAccessSourceV1, Error> {
    let path = WirePath::root();
    let owners = key.owners().owners().len() as u64;
    meter
        .check_semantic_depth(owners + 1, &path)
        .map_err(resource)?;
    meter
        .charge_collection_slots(owners, &path)
        .map_err(resource)?;
    work(meter, export.export_definition_origins.records().len())?;
    let origin = export
        .export_definition_origins
        .get(subject)
        .ok_or(Error::MissingDefinitionOrigin(subject))?;
    resources::name(origin.origin().source().logical_path().as_str(), meter)?;
    super::super::nominals::declaration_access_for_subject(export, key, subject, visibility.into())
}
