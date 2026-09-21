use super::*;

pub(super) fn verify(output: &hir::DependencyHirOutput, table: &Table) {
    let export = output.output().export.module();
    for record in table.records() {
        let access = record.declaration_access();
        assert_eq!(
            access.lexical_owners().last(),
            Some(&record.payload().owner())
        );
        let subject = match record.declaration() {
            CallableTemplateOrigin::Function(id) => DefinitionOriginSubject::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                DefinitionOriginSubject::GenericFunction(id)
            }
            CallableTemplateOrigin::Accessor(id) => DefinitionOriginSubject::PropertyAccessor(id),
            CallableTemplateOrigin::VariantConstructor(id) => {
                DefinitionOriginSubject::EnumVariant(id)
            }
            _ => panic!("nominal callable role"),
        };
        assert_eq!(
            access.definition_origin().origin(),
            export
                .export_definition_origins
                .get(subject)
                .unwrap()
                .origin()
        );
        if access.declared_visibility() == hir::DeclaredVisibilityV1::Private {
            assert_eq!(record.payload().modality(), hir::CallableModalityV1::Final);
            assert!(record.payload().slot_relations().is_empty());
        }
    }
    for (_, property) in export.properties.iter() {
        if matches!(
            property.representation,
            hir::PropertyRepresentation::Const { .. }
        ) {
            assert!(
                table
                    .get(CallableTemplateOrigin::Accessor(
                        export.property_accessor_identities[property.capability.getter()].id()
                    ))
                    .is_none()
            );
            continue;
        }
        let getter = CallableTemplateOrigin::Accessor(
            export.property_accessor_identities[property.capability.getter()].id(),
        );
        let Some(getter) = table.get(getter) else {
            continue;
        };
        assert!(getter.payload().parameters().parameters().is_empty());
        assert!(getter.payload().type_parameters().binders().is_empty());
        assert_eq!(
            getter.declaration_access().declared_visibility(),
            property.access.declared.into()
        );
        if matches!(export.types[property.ty], hir::Type::Param(_)) {
            assert_eq!(
                getter.payload().result(),
                &SignatureTypeKey::Binder { depth: 0, index: 0 }
            );
        }
        if let Some(setter) = property.capability.setter() {
            let source = table
                .get(CallableTemplateOrigin::Accessor(
                    export.property_accessor_identities[setter].id(),
                ))
                .unwrap();
            let parameter = &source.payload().parameters().parameters()[0];
            assert_eq!(
                parameter.name().as_str(),
                export.property_setters[setter].parameter_name
            );
            assert_eq!(parameter.value_type(), getter.payload().result());
            assert_eq!(
                source.declaration_access().declared_visibility(),
                export.property_setters[setter].access.declared.into()
            );
        }
    }
}
