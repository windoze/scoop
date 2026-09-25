use super::*;
use crate::{
    CallableDeclarationRecordV1, CallableInterfaceRecordV1, CanonicalNominalInterfacesV1,
    HirFunctionIdentity, HirSourceFunctionIdentity,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WirePath;

pub(super) fn project(
    projection: &CallableProjection<'_>,
    nominals: &CanonicalNominalInterfacesV1,
    public: &[CallableInterfaceRecordV1],
    top_level: &std::collections::BTreeMap<CallableTemplateOrigin, crate::PublicDeclarationOwnerV1>,
) -> Result<Vec<CallableDeclarationRecordV1>, CallableInterfaceBuildError> {
    use CallableInterfaceBuildError as Error;
    let mut required =
        CanonicalCallableInterfacesV1::required_declarations(nominals, projection.properties)
            .map_err(Error::Inventory)?;
    let path = WirePath::root().field(3);
    for (id, owner) in top_level {
        required.insert(*id, *owner);
    }
    for record in public {
        required.remove(&record.declaration());
    }
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, required.len(), &path)
        .map_err(Error::Resource)?;
    let export = projection.export;
    for (id, _) in export.functions.iter() {
        let declaration = match &export.function_identities[id] {
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
                CallableTemplateOrigin::Function(record.id())
            }
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(record)) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
            _ => continue,
        };

        if required.remove(&declaration).is_some() {
            records.push(functions::project(projection, id).map_err(|error| {
                Error::projection(CallableProjectionSubject::Function(raw_index(id)), error)
            })?);
        }
    }
    for (id, _) in export.struct_constructors.iter() {
        let declaration =
            CallableTemplateOrigin::Constructor(export.constructor_identities[id].id());

        if required.remove(&declaration).is_some() {
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
        let Some(source) = export.constructor_identities[id].source_record() else {
            continue;
        };
        let declaration = CallableTemplateOrigin::Constructor(source.id());

        if required.remove(&declaration).is_some() {
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
        let Some(source) = export.nominal_identities[id].source() else {
            continue;
        };
        let owner = crate::SourceNominalId::from_source_declaration(source.declaration())
            .map_err(|_| Error::InvalidSourceNominal)?;

        if !required.iter().any(|(declaration, required_owner)| {
            *required_owner == crate::PublicDeclarationOwnerV1::Nominal(owner)
                && matches!(declaration, CallableTemplateOrigin::VariantConstructor(_))
        }) {
            continue;
        }
        let mut variants = Vec::new();
        let enumeration = &export.enums[id];

        scoop_wire::allocation::try_reserve(&mut variants, enumeration.variants.len(), &path)
            .map_err(Error::Resource)?;
        for index in 0..enumeration.variants.len() {
            let index = u32::try_from(index).map_err(|_| Error::InvalidSourceNominal)?;
            crate::EnumVariantRef::checked(&export.enums, id, index)
                .ok_or(Error::InvalidSourceNominal)?;
        }
        variants::project_selected(projection, [id], &mut variants)?;
        for record in variants {
            if required.remove(&record.declaration()).is_some() {
                records.push(record);
            }
        }
    }
    for (id, _) in export.property_getters.iter() {
        let Some(identity) = export.property_accessor_identities.get_getter(id) else {
            continue;
        };
        let declaration = CallableTemplateOrigin::Accessor(identity.id());
        if required.remove(&declaration).is_some() {
            records.push(accessors::project_getter(projection, id).map_err(|error| {
                Error::projection(CallableProjectionSubject::Getter(raw_index(id)), error)
            })?);
        }
    }
    for (id, _) in export.property_setters.iter() {
        let Some(identity) = export.property_accessor_identities.get_setter(id) else {
            continue;
        };
        let declaration = CallableTemplateOrigin::Accessor(identity.id());
        if required.remove(&declaration).is_some() {
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
