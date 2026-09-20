use super::*;

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
    owner: SourceNominalId,
    meter: &mut BudgetMeter,
) -> Result<CanonicalNestedMemberRefsV1, Error> {
    let mut members = Vec::new();
    let (methods, properties): (&[FunctionId], &[PropertyId]) = match local {
        LocalNominalId::Class(id) => (&export.classes[id].methods, &export.classes[id].properties),
        LocalNominalId::Struct(id) => (&export.structs[id].methods, &export.structs[id].properties),
        LocalNominalId::Enum(id) => (&export.enums[id].methods, &export.enums[id].properties),
        LocalNominalId::Object(id) => {
            let d = &export.classes[export.objects[id].backing_class];
            (&d.methods, &d.properties)
        }
        LocalNominalId::Interface(id) => {
            let d = &export.interfaces[id];
            for &member in &d.methods {
                work(meter, 1)?;
                let method = &export.interface_methods[member];
                if method.owner != id {
                    return Err(invalid("source interface member has a different owner"));
                }
                match method.role {
                    InterfaceMemberRole::Function => {
                        function(export, owner, method.function, &mut members, meter)?
                    }
                    // Logical properties own their accessor relations separately.
                    InterfaceMemberRole::PropertyGetter(_)
                    | InterfaceMemberRole::PropertySetter(_) => continue,
                }
            }
            (&d.private_methods, &d.properties)
        }
    };
    for &id in methods {
        function(export, owner, id, &mut members, meter)?;
    }
    for &id in properties {
        work(meter, export.objects.len())?;
        let property = &export.properties[id];
        if owner_resolution::from_property(export, property.owner) != Some(owner) {
            return Err(invalid("nominal source property has a different HIR owner"));
        }
        let HirPropertyIdentity::Ordinary(record) = &export.property_identities[id] else {
            return Err(invalid("nominal source property has an extension identity"));
        };
        validate_owner(export, owner, record.key())?;
        push(
            &mut members,
            NestedSourceMemberRefV1::Property(record.id()),
            meter,
        )?;
    }
    charge_canonical(members.len(), meter)?;
    CanonicalNestedMemberRefsV1::try_new(members).map_err(invalid)
}

fn function(
    export: &ExportHir,
    owner: SourceNominalId,
    id: FunctionId,
    members: &mut Vec<NestedSourceMemberRefV1>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    work(meter, 1)?;
    let source = match &export.function_identities[id] {
        HirFunctionIdentity::Source(source) => source,
        HirFunctionIdentity::PropertyAccessor(_)
        | HirFunctionIdentity::LexicalGenerated(_)
        | HirFunctionIdentity::Initialization { .. }
        | HirFunctionIdentity::DerivedEquality(_) => return Ok(()),
    };
    let method = export.functions[id]
        .method
        .ok_or_else(|| invalid("nominal source function has no HIR method owner"))?;
    meter
        .charge_work(export.objects.len() as u64, &WirePath::root())
        .map_err(resource)?;
    if owner_resolution::from_type(export, method.owner) != Some(owner) {
        return Err(invalid("nominal source method has a different HIR owner"));
    }
    validate_owner(export, owner, source.declaration())?;
    let reference = match source {
        HirSourceFunctionIdentity::Plain(record) => NestedSourceMemberRefV1::Function(record.id()),
        HirSourceFunctionIdentity::Generic(record) => {
            NestedSourceMemberRefV1::GenericFunction(record.id())
        }
    };
    push(members, reference, meter)
}
