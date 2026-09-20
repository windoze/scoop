use super::*;

fn names(export: &hir::ExportHir) -> BTreeMap<CallableTemplateOrigin, String> {
    let mut result = BTreeMap::new();
    for (id, function) in export.functions.iter() {
        let origin = match &export.function_identities[id] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                CallableTemplateOrigin::Function(record.id())
            }
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
            _ => continue,
        };
        result.insert(origin, function.name.clone());
    }
    for (_, property) in export.properties.iter() {
        let owner = match property.owner {
            hir::PropertyOwner::Class(id) => &export.classes[id].name,
            hir::PropertyOwner::Interface(id) => &export.interfaces[id].name,
            hir::PropertyOwner::Struct(id) => &export.structs[id].name,
            hir::PropertyOwner::Enum(id) => &export.enums[id].name,
            hir::PropertyOwner::Object(id) => &export.objects[id].name,
            _ => continue,
        };
        result.insert(
            CallableTemplateOrigin::Accessor(
                export.property_accessor_identities[property.capability.getter()].id(),
            ),
            format!("{owner}.{}.get", property.name),
        );
        if let Some(setter) = property.capability.setter() {
            result.insert(
                CallableTemplateOrigin::Accessor(export.property_accessor_identities[setter].id()),
                format!("{owner}.{}.set", property.name),
            );
        }
    }
    for (id, enumeration) in export.enums.iter() {
        for (index, variant) in enumeration.variants.iter().enumerate() {
            let reference = hir::EnumVariantRef::checked(&export.enums, id, index as u32).unwrap();
            result.insert(
                CallableTemplateOrigin::VariantConstructor(
                    export.enum_member_identities[reference].id(),
                ),
                format!("{}.{}", enumeration.name, variant.name),
            );
        }
    }
    result
}

fn shape(ty: &SignatureTypeKey) -> String {
    match ty {
        SignatureTypeKey::Binder { depth, index } => format!("binder({depth},{index})"),
        SignatureTypeKey::Nominal(_) => "nominal".into(),
        SignatureTypeKey::NominalApplication { arguments, .. } => format!(
            "application<{}>",
            arguments
                .as_slice()
                .iter()
                .map(shape)
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => panic!("unexpected fixture type {other:?}"),
    }
}

#[test]
fn complete_nominal_callable_source_dump_is_stable() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let names = names(output.output().export.module());
        let mut rows = table
            .records()
            .iter()
            .map(|record| {
                let payload = record.payload();
                let access = record.declaration_access();
                let parameters = payload
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|p| format!("{}:{}", p.name().as_str(), shape(p.value_type())))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{}({parameters})->{}: {:?} {:?} {:?}, own-binders={}, owners={}, slots={}\n",
                    names[&record.declaration()],
                    shape(payload.result()),
                    access.declared_visibility(),
                    payload.modality(),
                    payload.effects().safety(),
                    payload.type_parameters().len_u32(),
                    access.lexical_owners().len(),
                    payload.slot_relations().slots().len()
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        assert_eq!(
            rows.concat(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-nominals/callables.snap"
            ))
        );
    });
}
