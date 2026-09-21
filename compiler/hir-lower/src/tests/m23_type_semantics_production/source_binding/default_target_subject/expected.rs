use super::*;

pub(super) fn targets(export: &hir::ExportHir) -> Vec<(Target, Subject, &hir::AccessDomain)> {
    let mut targets = Vec::new();
    for (id, declaration) in export.structs.iter() {
        let identity = export.nominal_identities[id].source().unwrap();
        if identity.declaration().origin() != export.cone {
            continue;
        }
        for index in 0..declaration.semantic_fields().len() {
            let field = hir::StructFieldRef::checked(&export.structs, id, index as u32).unwrap();
            targets.push((
                Target::StructField(export.field_identities[field].id()),
                subject(identity),
                &declaration.access.lookup.0,
            ));
        }
    }
    for (id, field) in export.class_fields.iter() {
        let identity = &export.property_identities[field.property];
        if identity.declaration().origin() != export.cone {
            continue;
        }
        let hir::HirPropertyIdentity::Ordinary(identity) = identity else {
            panic!("ordinary field property");
        };
        targets.push((
            Target::ClassField(export.field_identities[id].id()),
            Subject::Property(identity.id()),
            &export.properties[field.property].access.lookup.0,
        ));
    }
    for (id, declaration) in export.enums.iter() {
        let identity = export.nominal_identities[id].source().unwrap();
        if identity.declaration().origin() != export.cone {
            continue;
        }
        for index in 0..declaration.variants.len() {
            let variant = hir::EnumVariantRef::checked(&export.enums, id, index as u32).unwrap();
            targets.push((
                Target::EnumVariant(export.enum_member_identities[variant].id()),
                subject(identity),
                &declaration.access.lookup.0,
            ));
        }
    }
    for (id, declaration) in export.objects.iter() {
        let identity = export.nominal_identities[id].source().unwrap();
        if identity.declaration().origin() != export.cone {
            continue;
        }
        targets.push((
            Target::Singleton(export.object_value_identities[declaration.singleton_value].id()),
            subject(identity),
            &declaration.access.lookup.0,
        ));
    }
    targets
}
fn subject(identity: &hir::HirSourceNominalIdentity) -> Subject {
    match identity {
        hir::HirSourceNominalIdentity::Concrete(record) => Subject::Type(record.id()),
        hir::HirSourceNominalIdentity::Generic(record) => Subject::GenericType(record.id()),
    }
}
