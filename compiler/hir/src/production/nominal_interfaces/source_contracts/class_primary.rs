use super::*;

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
) -> Result<Option<ClassPrimaryConstructorV1>, Error> {
    let LocalNominalId::Class(class) = local else {
        return Ok(None);
    };
    let declaration = &export.classes[class];
    let primary = declaration.constructors.iter().copied().find(|&id| {
        let constructor = &export.class_constructors[id];
        constructor.identity_kind == ClassConstructorIdentityKind::Source
            && matches!(constructor.kind, ClassConstructorKind::Primary { .. })
    });
    let Some(primary) = primary else {
        return Ok(None);
    };
    let constructor = &export.class_constructors[primary];
    let identity = export.constructor_identities[primary]
        .source_record()
        .ok_or_else(|| invalid("class primary constructor has no source identity"))?;
    let mut properties = vec![None; constructor.parameters.len()];
    for &field in &declaration.fields {
        let field = &export.class_fields[field];
        let ClassFieldSource::PrimaryParameter(parameter) = field.source else {
            continue;
        };
        let index = constructor
            .parameters
            .iter()
            .position(|candidate| candidate.id == parameter)
            .ok_or_else(|| invalid("primary property refers to a missing constructor parameter"))?;
        let property = export.property_identities[field.property]
            .ordinary_id()
            .ok_or_else(|| {
                invalid("primary parameter must refer to an ordinary logical property")
            })?;
        if properties[index].replace(property).is_some() {
            return Err(invalid(
                "primary constructor parameter initializes multiple properties",
            ));
        }
    }
    ClassPrimaryConstructorV1::try_new(identity.id(), properties)
        .map(Some)
        .map_err(invalid)
}
