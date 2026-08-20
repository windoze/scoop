use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn register_imported_companion(
        &mut self,
        template: export::ImportedCompanionTemplateId,
        backing_class: concrete::ClassId,
    ) {
        let declaration = &self.source.imported_companion_templates[template];
        let source = &declaration.declaration;
        let export::NominalSourceShapeV1::Object(shape) = source.interface.source_shape() else {
            unreachable!("imported companions retain their object source shape")
        };
        let object = concrete::ObjectId::from_raw((self.objects.len() as u32).into());
        let canonical_type = self.class_type[&backing_class];
        let object_type = self.object_types.alloc(concrete::ObjectType {
            declaration: object,
            representation: backing_class,
            canonical_type,
        });
        self.object_type_map.insert(backing_class, object_type);
        let value =
            concrete::SingletonValueId::from_raw((self.singleton_values.len() as u32).into());
        let published_root =
            self.singleton_published_roots
                .alloc(concrete::SingletonPublishedRoot {
                    value,
                    ty: canonical_type,
                });
        self.singleton_root_map.insert(value, published_root);
        let initialization = self.request_initialization(InitializationKey {
            source: InitializationSource::ImportedCompanion(template),
            arguments: self.classes[backing_class].type_arguments.clone(),
        });
        assert_eq!(
            self.singleton_values.alloc(concrete::SingletonValue {
                identity: shape.value(),
                declaration: object,
                object_type,
                published_root,
                initialization,
            }),
            value
        );
        self.singleton_value_map.insert(object_type, value);
        let host = &declaration.host;
        let origin = export::HirNominalIdentity::Source(host.identity.clone());
        let host = match host.identity.declaration().declaration_kind() {
            scoop_identity::SourceDeclarationKind::Class => concrete::NominalOwner::Class(origin),
            scoop_identity::SourceDeclarationKind::Struct => concrete::NominalOwner::Struct(origin),
            scoop_identity::SourceDeclarationKind::Enum => concrete::NominalOwner::Enum(origin),
            scoop_identity::SourceDeclarationKind::Interface => {
                concrete::NominalOwner::Interface(origin)
            }
            scoop_identity::SourceDeclarationKind::Object => concrete::NominalOwner::Object(origin),
            _ => unreachable!("a companion has a nominal host"),
        };
        let relation = self.companion_relations.alloc(concrete::CompanionRelation {
            host: host.clone(),
            object,
            name: if source.name() == "Companion" {
                concrete::CompanionName::Default
            } else {
                concrete::CompanionName::Named(source.name().to_owned())
            },
        });
        assert_eq!(
            self.objects.alloc(concrete::ObjectDecl {
                origin: export::HirNominalIdentity::Source(source.identity.clone()),
                name: source.name().to_owned(),
                owner: Some(host),
                object_type,
                singleton_value: value,
                kind: concrete::ObjectKind::Companion(relation),
                backing_class,
                span: declaration.origin.span,
            }),
            object
        );
    }
}
