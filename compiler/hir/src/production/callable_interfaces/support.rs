use super::*;
use crate::{
    CallableDeclarationRecordV1, CallableInterfaceRecordV1, CanonicalNominalInterfacesV1,
    HirFunctionIdentity, HirSourceFunctionIdentity,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};

mod resources;

fn query(meter: &mut BudgetMeter, count: usize) -> Result<(), CallableInterfaceBuildError> {
    meter
        .charge_work(
            u64::from(count.max(1).ilog2()) + 1,
            &WirePath::root().field(3),
        )
        .map_err(CallableInterfaceBuildError::Resource)
}

pub(super) fn project(
    projection: &CallableProjection<'_>,
    nominals: &CanonicalNominalInterfacesV1,
    public: &[CallableInterfaceRecordV1],
    top_level: &std::collections::BTreeMap<CallableTemplateOrigin, crate::PublicDeclarationOwnerV1>,
    meter: &mut BudgetMeter,
) -> Result<Vec<CallableDeclarationRecordV1>, CallableInterfaceBuildError> {
    use CallableInterfaceBuildError as Error;
    let mut required = CanonicalCallableInterfacesV1::required_declarations(
        nominals,
        projection.properties,
        meter,
    )
    .map_err(Error::Inventory)?;
    let path = WirePath::root().field(3);
    for (id, owner) in top_level {
        query(meter, required.len())?;
        meter
            .charge_collection_slots(1, &path)
            .map_err(Error::Resource)?;
        required.insert(*id, *owner);
    }
    for record in public {
        query(meter, required.len())?;
        required.remove(&record.declaration());
    }
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(&mut records, required.len(), &path)
        .map_err(Error::Resource)?;
    let export = projection.export;
    for (id, _) in export.functions.iter() {
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let declaration = match &export.function_identities[id] {
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
                CallableTemplateOrigin::Function(record.id())
            }
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(record)) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
            _ => continue,
        };
        query(meter, required.len())?;
        if required.remove(&declaration).is_some() {
            resources::function(export, id, meter).map_err(Error::Resource)?;
            records.push(functions::project(projection, id).map_err(|error| {
                Error::projection(CallableProjectionSubject::Function(raw_index(id)), error)
            })?);
        }
    }
    for (id, _) in export.struct_constructors.iter() {
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let declaration =
            CallableTemplateOrigin::Constructor(export.constructor_identities[id].id());
        query(meter, required.len())?;
        if required.remove(&declaration).is_some() {
            let constructor = &export.struct_constructors[id];
            let owner = &export.structs[constructor.owner];
            resources::nominal(
                export,
                &owner.type_params,
                crate::ExportParameterOwner::StructConstructor(id),
                export.struct_applications[owner.self_application].canonical_type,
                meter,
            )
            .map_err(Error::Resource)?;
            records.push(
                constructors::project_struct(projection, id).map_err(|error| {
                    Error::projection(
                        CallableProjectionSubject::StructConstructor(raw_index(id)),
                        error,
                    )
                })?,
            );
        }
    }
    for (id, _) in export.class_constructors.iter() {
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let Some(source) = export.constructor_identities[id].source_record() else {
            continue;
        };
        let declaration = CallableTemplateOrigin::Constructor(source.id());
        query(meter, required.len())?;
        if required.remove(&declaration).is_some() {
            let constructor = &export.class_constructors[id];
            let owner = &export.classes[constructor.owner];
            resources::nominal(
                export,
                &owner.type_params,
                crate::ExportParameterOwner::ClassConstructor(id),
                export.class_applications[owner.self_application].canonical_type,
                meter,
            )
            .map_err(Error::Resource)?;
            records.push(
                constructors::project_class(projection, id)
                    .map_err(|error| {
                        Error::projection(
                            CallableProjectionSubject::ClassConstructor(raw_index(id)),
                            error,
                        )
                    })?
                    .ok_or(Error::MissingSupport(declaration))?,
            );
        }
    }
    for (id, _) in export.enums.iter() {
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let Some(source) = export.nominal_identities[id].source() else {
            continue;
        };
        let owner = crate::SourceNominalId::from_source_declaration(source.declaration())
            .map_err(|_| Error::InvalidSourceNominal)?;
        meter
            .charge_work(required.len() as u64, &path)
            .map_err(Error::Resource)?;
        if !required.iter().any(|(declaration, required_owner)| {
            *required_owner == crate::PublicDeclarationOwnerV1::Nominal(owner)
                && matches!(declaration, CallableTemplateOrigin::VariantConstructor(_))
        }) {
            continue;
        }
        let mut variants = Vec::new();
        let enumeration = &export.enums[id];
        meter
            .check_table_entries(enumeration.variants.len() as u64, &path)
            .map_err(Error::Resource)?;
        meter
            .try_reserve_collection_slots(&mut variants, enumeration.variants.len(), &path)
            .map_err(Error::Resource)?;
        for index in 0..enumeration.variants.len() {
            let index = u32::try_from(index).map_err(|_| Error::InvalidSourceNominal)?;
            let reference = crate::EnumVariantRef::checked(&export.enums, id, index)
                .ok_or(Error::InvalidSourceNominal)?;
            resources::nominal(
                export,
                &enumeration.type_params,
                crate::ExportParameterOwner::VariantConstructor(reference),
                export.enum_applications[enumeration.self_application].canonical_type,
                meter,
            )
            .map_err(Error::Resource)?;
        }
        variants::project_selected(projection, [id], &mut variants)?;
        for record in variants {
            query(meter, required.len())?;
            if required.remove(&record.declaration()).is_some() {
                records.push(record);
            }
        }
    }
    for (id, _) in export.property_getters.iter() {
        query(meter, required.len())?;
        let Some(identity) = export.property_accessor_identities.get_getter(id) else {
            continue;
        };
        let declaration = CallableTemplateOrigin::Accessor(identity.id());
        if required.remove(&declaration).is_some() {
            resources::accessor(export, identity.property(), None, meter)
                .map_err(Error::Resource)?;
            records.push(accessors::project_getter(projection, id).map_err(|error| {
                Error::projection(CallableProjectionSubject::Getter(raw_index(id)), error)
            })?);
        }
    }
    for (id, setter) in export.property_setters.iter() {
        query(meter, required.len())?;
        let Some(identity) = export.property_accessor_identities.get_setter(id) else {
            continue;
        };
        let declaration = CallableTemplateOrigin::Accessor(identity.id());
        if required.remove(&declaration).is_some() {
            resources::accessor(
                export,
                identity.property(),
                Some(&setter.parameter_name),
                meter,
            )
            .map_err(Error::Resource)?;
            records.push(accessors::project_setter(projection, id).map_err(|error| {
                Error::projection(CallableProjectionSubject::Setter(raw_index(id)), error)
            })?);
        }
    }
    if let Some((declaration, _)) = required.first_key_value() {
        return Err(Error::MissingSupport(*declaration));
    }
    Ok(records)
}
