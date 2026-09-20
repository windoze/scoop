use super::*;
use crate::production::type_semantics::nominals::{identity, nominal_access};
use scoop_identity::{CallableTemplateOrigin, DefinitionOwnerAtom, SourceDeclarationKey};
use std::collections::BTreeMap;

type Members = BTreeMap<SourceNominalId, Vec<ProtectedDeclarationRefV1>>;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    meter: &mut BudgetMeter,
) -> Result<Members, Error> {
    let mut members = BTreeMap::new();
    for (id, function) in export.functions.iter() {
        meter.charge_work(1, &WirePath::root()).map_err(resource)?;
        if function.access.declared != DeclaredVisibility::Protected {
            continue;
        }
        let source = match &export.function_identities[id] {
            HirFunctionIdentity::Source(source) => source,
            HirFunctionIdentity::PropertyAccessor(_)
            | HirFunctionIdentity::LexicalGenerated(_)
            | HirFunctionIdentity::Initialization { .. }
            | HirFunctionIdentity::DerivedEquality(_) => continue,
        };
        let declaration = match source {
            HirSourceFunctionIdentity::Plain(record) => {
                CallableTemplateOrigin::Function(record.id())
            }
            HirSourceFunctionIdentity::Generic(record) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
        };
        insert(
            &mut members,
            source.declaration(),
            callable(declaration)?,
            meter,
        )?;
    }
    for (id, property) in export.properties.iter() {
        meter.charge_work(1, &WirePath::root()).map_err(resource)?;
        let getter_protected = property.access.declared == DeclaredVisibility::Protected;
        let setter_protected = property.capability.setter().is_some_and(|setter| {
            export.property_setters[setter].access.declared == DeclaredVisibility::Protected
        });
        if !getter_protected && !setter_protected {
            continue;
        }
        let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            return Err(invalid(
                "protected member property has an extension identity",
            ));
        };
        if getter_protected {
            insert(
                &mut members,
                identity.key(),
                ProtectedDeclarationRefV1::Property(identity.id()),
                meter,
            )?;
        }
        if matches!(
            property.representation,
            PropertyRepresentation::Const { .. }
        ) {
            continue;
        }
        if getter_protected {
            let accessor = export.property_accessor_identities[property.capability.getter()]
                .record()
                .id();
            insert(
                &mut members,
                identity.key(),
                callable(CallableTemplateOrigin::Accessor(accessor))?,
                meter,
            )?;
        }
        if let Some(setter) = property.capability.setter()
            && setter_protected
        {
            let accessor = export.property_accessor_identities[setter].record().id();
            insert(
                &mut members,
                identity.key(),
                callable(CallableTemplateOrigin::Accessor(accessor))?,
                meter,
            )?;
        }
    }
    for nominal in all_nominals(export) {
        meter.charge_work(1, &WirePath::root()).map_err(resource)?;
        if nominal_access(export, nominal).declared != DeclaredVisibility::Protected {
            continue;
        }
        // Object backing classes inherit access metadata, but only the source
        // object is a nested declaration in the protected member inventory.
        let Some(source) = identity(export, nominal)?.source() else {
            continue;
        };
        let declaration = SourceNominalId::from_source_declaration(source.declaration())
            .map_err(|error| invalid(error.to_string()))?;
        insert(
            &mut members,
            source.declaration(),
            ProtectedDeclarationRefV1::NestedNominal(declaration),
            meter,
        )?;
    }
    Ok(members)
}

fn callable(declaration: CallableTemplateOrigin) -> Result<ProtectedDeclarationRefV1, Error> {
    ProtectedCallableDeclarationRefV1::try_new(declaration)
        .map(ProtectedDeclarationRefV1::Callable)
        .map_err(|error| invalid(error.to_string()))
}

fn insert(
    members: &mut Members,
    key: &SourceDeclarationKey,
    declaration: ProtectedDeclarationRefV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let owner = match key.owners().owners().last() {
        Some(DefinitionOwnerAtom::Type(id)) => SourceNominalId::Concrete(*id),
        Some(DefinitionOwnerAtom::GenericType(id)) => SourceNominalId::GenericTemplate(*id),
        _ => {
            return Err(invalid(
                "protected declaration has no lexical nominal owner",
            ));
        }
    };
    let path = WirePath::root();
    meter
        .charge_work(u64::from(members.len().max(1).ilog2()) + 1, &path)
        .map_err(resource)?;
    meter.charge_collection_slots(1, &path).map_err(resource)?;
    let declarations = members.entry(owner).or_default();
    meter
        .check_table_entries((declarations.len() as u64).saturating_add(1), &path)
        .map_err(resource)?;
    meter
        .try_reserve_collection_slots(declarations, 1, &path)
        .map_err(resource)?;
    declarations.push(declaration);
    Ok(())
}

fn all_nominals(export: &ExportHir) -> impl Iterator<Item = NominalLocalId> + '_ {
    export
        .structs
        .iter()
        .map(|(id, _)| NominalLocalId::Struct(id))
        .chain(export.enums.iter().map(|(id, _)| NominalLocalId::Enum(id)))
        .chain(
            export
                .classes
                .iter()
                .map(|(id, _)| NominalLocalId::Class(id)),
        )
        .chain(
            export
                .interfaces
                .iter()
                .map(|(id, _)| NominalLocalId::Interface(id)),
        )
        .chain(
            export
                .objects
                .iter()
                .map(|(id, _)| NominalLocalId::Object(id)),
        )
}

fn invalid(reason: impl Into<String>) -> Error {
    Error::InvalidSourceDeclaration(reason.into())
}
