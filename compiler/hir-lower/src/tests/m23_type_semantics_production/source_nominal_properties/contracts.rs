use super::*;

pub(super) fn verify(output: &hir::DependencyHirOutput, table: &Table) {
    let export = output.output().export.module();
    for (id, property) in export.properties.iter() {
        let hir::HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        let Some(record) = table.get(identity.id()) else {
            continue;
        };
        let access = record.declaration_access();
        assert_eq!(
            access.declared_visibility(),
            property.access.declared.into()
        );
        assert_eq!(access.lexical_owners().last(), Some(&record.owner()));
        assert_eq!(
            access.definition_origin().origin(),
            export
                .export_definition_origins
                .get(DefinitionOriginSubject::Property(identity.id()))
                .unwrap()
                .origin()
        );
        match record.payload() {
            Payload::Const { value } => {
                let hir::PropertyRepresentation::Const { value: actual } = &property.representation
                else {
                    panic!("source const");
                };
                assert_eq!(
                    value.value(),
                    &hir::CanonicalConstValueV1::from(actual.clone())
                );
                assert_eq!(value.definition_origin(), access.definition_origin());
            }
            Payload::Runtime { interface } => {
                assert_eq!(interface.owner(), record.owner());
                assert_eq!(
                    interface.getter(),
                    export.property_accessor_identities[property.capability.getter()].id()
                );
                if matches!(export.types[property.ty], hir::Type::Param(_)) {
                    assert_eq!(
                        interface.value_type(),
                        &SignatureTypeKey::Binder { depth: 0, index: 0 }
                    );
                }
                match (interface.mutability(), property.capability.setter()) {
                    (hir::ProtectedPropertyMutabilityV1::ReadOnly, None) => {
                        assert!(property.capability.setter().is_none())
                    }
                    (
                        hir::ProtectedPropertyMutabilityV1::ReadWrite {
                            setter,
                            setter_access,
                        },
                        Some(local),
                    ) => {
                        assert_eq!(*setter, export.property_accessor_identities[local].id());
                        assert_eq!(
                            setter_access.declared_visibility(),
                            export.property_setters[local].access.declared.into()
                        );
                        assert_eq!(
                            setter_access.definition_origin().origin(),
                            export
                                .export_definition_origins
                                .get(DefinitionOriginSubject::PropertyAccessor(*setter))
                                .unwrap()
                                .origin()
                        );
                    }
                    _ => panic!("source mutability"),
                }
            }
        }
    }
}

pub(super) fn render(output: &hir::DependencyHirOutput, table: &Table) -> String {
    let export = output.output().export.module();
    let mut lines = Vec::new();
    for (id, property) in export.properties.iter() {
        let hir::HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        let Some(record) = table.get(identity.id()) else {
            continue;
        };
        let owner = match property.owner {
            hir::PropertyOwner::Class(id) => &export.classes[id].name,
            hir::PropertyOwner::Interface(id) => &export.interfaces[id].name,
            hir::PropertyOwner::Struct(id) => &export.structs[id].name,
            hir::PropertyOwner::Enum(id) => &export.enums[id].name,
            hir::PropertyOwner::Object(id) => &export.objects[id].name,
            _ => panic!("nominal owner"),
        };
        let payload = match record.payload() {
            Payload::Const { value } => match value.value() {
                hir::CanonicalConstValueV1::Integer(value) => format!("Const({value:?})"),
                hir::CanonicalConstValueV1::Boolean(value) => format!("Const({})", value.value()),
                hir::CanonicalConstValueV1::String(value) => format!("Const({value:?})"),
            },
            Payload::Runtime { interface } => {
                let value = if matches!(
                    interface.value_type(),
                    SignatureTypeKey::Binder { depth: 0, index: 0 }
                ) {
                    "Binder(0,0)"
                } else {
                    "Nominal"
                };
                let setter = match interface.mutability() {
                    hir::ProtectedPropertyMutabilityV1::ReadOnly => "ReadOnly".into(),
                    hir::ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } => {
                        format!("ReadWrite({:?})", setter_access.declared_visibility())
                    }
                };
                format!(
                    "{value}, {setter}, {:?}, slots={}",
                    interface.representation(),
                    interface.slot_relations().slots().len()
                )
            }
        };
        lines.push(format!(
            "{owner}.{}: {:?}, {payload}\n",
            property.name,
            record.declaration_access().declared_visibility()
        ));
    }
    lines.sort();
    lines.concat()
}
