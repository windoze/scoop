use super::source_resources::{self as resources, invalid, resource, work};
use super::*;
use crate::production::{
    property_interfaces::source_property_representation, signatures::HirInterfaceSignatureProjector,
};
use scoop_identity::{DefinitionOriginSubject, PersistentPropertyId, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};

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
    for (id, property) in export.properties.iter() {
        work(meter, required.len())?;
        let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        if !required.remove(&identity.id()) {
            continue;
        }
        let key = identity.key();
        if key.origin() != export.cone {
            return Err(invalid("source property belongs to another cone"));
        }
        resources::ty(export, property.ty, 0, 3, meter)?;
        let value_type = signatures.map_type(property.ty, &[]).map_err(invalid)?;
        let access = declaration_access(
            export,
            key,
            DefinitionOriginSubject::Property(identity.id()),
            property.access.declared,
            meter,
        )?;
        let owner = access
            .lexical_owners()
            .last()
            .copied()
            .ok_or_else(|| invalid("inheritance property has no nominal source owner"))?;
        let getter_id = property.capability.getter();
        let getter = &export.property_getters[getter_id];
        let setter = property
            .capability
            .setter()
            .map(|id| &export.property_setters[id]);
        let representation =
            source_property_representation(property, getter, setter).map_err(invalid)?;
        if representation == PropertyRepresentationV1::Const {
            return Err(invalid(
                "const property cannot be a protected or dispatch accessor source",
            ));
        }
        let mutability = match property.capability.setter() {
            None => ProtectedPropertyMutabilityV1::ReadOnly,
            Some(setter_id) => {
                let setter = &export.property_setters[setter_id];
                let id = export.property_accessor_identities[setter_id].id();
                ProtectedPropertyMutabilityV1::ReadWrite {
                    setter: id,
                    setter_access: declaration_access(
                        export,
                        key,
                        DefinitionOriginSubject::PropertyAccessor(id),
                        setter.access.declared,
                        meter,
                    )?,
                }
            }
        };
        let interface = NominalSourcePropertyPayloadV1::try_new(
            owner,
            value_type,
            export.property_accessor_identities[getter_id].id(),
            mutability,
            representation,
            slots::project(
                export,
                getter.implementation,
                setter.map(|setter| setter.implementation),
                meter,
            )?,
        )
        .map_err(invalid)?;
        meter
            .check_table_entries(records.len() as u64 + 1, &WirePath::root())
            .map_err(resource)?;
        meter
            .try_reserve_collection_slots(&mut records, 1, &WirePath::root())
            .map_err(resource)?;
        records.push(
            NominalSupportPropertyInterfaceV1::try_new(
                identity.id(),
                access,
                NominalSupportPropertyPayloadV1::Runtime { interface },
            )
            .map_err(invalid)?,
        );
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
