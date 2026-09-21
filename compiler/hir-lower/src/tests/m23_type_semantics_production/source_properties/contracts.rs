use super::*;

pub(super) fn verify(output: &hir::DependencyHirOutput, table: &Table) {
    let export = output.output().export.module();
    let mapper = hir::HirSignatureTypeMapper::new(hir::HirTypeIdentityInputs::from_export(export));
    for (id, property) in export.properties.iter() {
        let hir::HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            continue;
        };
        let Some(record) = table.get(identity.id()) else {
            continue;
        };
        assert_eq!(
            record.declaration_access().declared_visibility(),
            property.access.declared.into()
        );
        assert_eq!(
            record.declaration_access().definition_origin().origin(),
            export
                .export_definition_origins
                .get(DefinitionOriginSubject::Property(identity.id()))
                .unwrap()
                .origin()
        );
        let Payload::Runtime { interface } = record.payload() else {
            panic!("runtime property");
        };
        assert_eq!(
            interface.value_type(),
            &mapper.map(property.ty, &[]).unwrap()
        );
        assert_eq!(record.owner(), interface.owner());
        let getter = export.property_accessor_identities[property.capability.getter()].id();
        assert_eq!(interface.getter(), getter);
        match (interface.mutability(), property.capability.setter()) {
            (hir::ProtectedPropertyMutabilityV1::ReadOnly, None) => continue,
            (
                hir::ProtectedPropertyMutabilityV1::ReadWrite {
                    setter,
                    setter_access,
                },
                Some(local),
            ) => {
                assert_eq!(*setter, export.property_accessor_identities[local].id());
                assert_ne!(*setter, getter);
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
                assert_eq!(
                    setter_access.lexical_owners(),
                    record.declaration_access().lexical_owners()
                );
            }
            _ => panic!("source mutability mismatch"),
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
            _ => panic!("nominal property owner"),
        };
        let Payload::Runtime { interface } = record.payload() else {
            panic!("runtime property");
        };
        let mutability = match interface.mutability() {
            hir::ProtectedPropertyMutabilityV1::ReadOnly => "ReadOnly".into(),
            hir::ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. } => {
                format!("ReadWrite({:?})", setter_access.declared_visibility())
            }
        };
        lines.push(format!(
            "{owner}.{}: {:?}, {mutability}, {:?}, slots={}\n",
            property.name,
            record.declaration_access().declared_visibility(),
            interface.representation(),
            interface.slot_relations().slots().len()
        ));
    }
    lines.sort();
    lines.concat()
}
