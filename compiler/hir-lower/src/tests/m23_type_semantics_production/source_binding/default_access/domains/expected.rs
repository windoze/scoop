use super::*;

pub(super) fn lookups(export: &hir::ExportHir) -> BTreeMap<Subject, &hir::AccessDomain> {
    let mut result = BTreeMap::new();
    for (id, nominal) in export.classes.iter() {
        insert_nominal(
            &mut result,
            &export.nominal_identities[id],
            &nominal.access.lookup.0,
        );
    }
    for (id, nominal) in export.structs.iter() {
        insert_nominal(
            &mut result,
            &export.nominal_identities[id],
            &nominal.access.lookup.0,
        );
    }
    for (id, nominal) in export.enums.iter() {
        insert_nominal(
            &mut result,
            &export.nominal_identities[id],
            &nominal.access.lookup.0,
        );
    }
    for (id, nominal) in export.interfaces.iter() {
        insert_nominal(
            &mut result,
            &export.nominal_identities[id],
            &nominal.access.lookup.0,
        );
    }
    for (id, nominal) in export.objects.iter() {
        insert_nominal(
            &mut result,
            &export.nominal_identities[id],
            &nominal.access.lookup.0,
        );
    }
    for (id, source) in export.functions.iter() {
        let subject = match &export.function_identities[id] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                Subject::Function(record.id())
            }
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => {
                Subject::GenericFunction(record.id())
            }
            _ => continue,
        };
        result.insert(subject, &source.access.lookup.0);
    }
    for (id, property) in export.properties.iter() {
        let subject = match &export.property_identities[id] {
            hir::HirPropertyIdentity::Ordinary(record) => Subject::Property(record.id()),
            hir::HirPropertyIdentity::Extension(record) => Subject::ExtensionProperty(record.id()),
        };
        result.insert(subject, &property.access.lookup.0);
        let getter = property.capability.getter();
        result.insert(
            Subject::PropertyAccessor(export.property_accessor_identities[getter].id()),
            &export.property_getters[getter].access.lookup.0,
        );
        if let Some(setter) = property.capability.setter() {
            result.insert(
                Subject::PropertyAccessor(export.property_accessor_identities[setter].id()),
                &export.property_setters[setter].access.lookup.0,
            );
        }
    }
    for (id, source) in export.struct_constructors.iter() {
        result.insert(
            Subject::Constructor(export.constructor_identities[id].id()),
            &source.access.lookup.0,
        );
    }
    for (id, source) in export.class_constructors.iter() {
        if let Some(record) = export.constructor_identities[id].source_record() {
            result.insert(Subject::Constructor(record.id()), &source.access.lookup.0);
        }
    }
    result
}
fn insert_nominal<'a>(
    result: &mut BTreeMap<Subject, &'a hir::AccessDomain>,
    identity: &hir::HirNominalIdentity,
    domain: &'a hir::AccessDomain,
) {
    let subject = match identity.source() {
        Some(hir::HirSourceNominalIdentity::Concrete(record)) => Subject::Type(record.id()),
        Some(hir::HirSourceNominalIdentity::Generic(record)) => Subject::GenericType(record.id()),
        None => return,
    };
    result.insert(subject, domain);
}
